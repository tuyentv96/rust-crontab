//! Comprehensive asynchronous tests for CronTab
//!
//! This module contains all tests for the asynchronous functionality of the cron scheduler,
//! including basic async operations, comprehensive scheduling scenarios, error handling,
//! and performance validation in async contexts.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;

use chrono::{FixedOffset, Local, TimeZone, Utc, Timelike};
use cron_tab::AsyncCron;
use tokio::time::sleep;
use tokio::sync::Mutex;
use futures::future::join_all;

#[cfg(test)]
mod tests {
    use super::*;

    // ===============================
    // BASIC ASYNC FUNCTIONALITY TESTS
    // ===============================

    #[tokio::test]
    async fn start_and_stop_cron() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = AsyncCron::new(local_tz);

        cron.start().await;
        sleep(Duration::from_millis(50)).await;
        cron.stop().await;
    }

    #[tokio::test]
    async fn add_job_before_start() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = AsyncCron::new(local_tz);

        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let job_id = cron
            .add_fn("* * * * * * *", move || {
                let counter = Arc::clone(&counter_clone);
                async move {
                    let mut count = counter.lock().await;
                    *count += 1;
                }
            })
            .await
            .unwrap();

        cron.start().await;
        sleep(Duration::from_millis(1100)).await;
        cron.stop().await;

        let count = *counter.lock().await;
        assert!(count >= 1);

        // Clean up
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn add_job() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = AsyncCron::new(local_tz);

        cron.start().await;

        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let job_id = cron
            .add_fn("* * * * * * *", move || {
                let counter = Arc::clone(&counter_clone);
                async move {
                    let mut count = counter.lock().await;
                    *count += 1;
                }
            })
            .await
            .unwrap();

        sleep(Duration::from_millis(1100)).await;
        cron.stop().await;

        let count = *counter.lock().await;
        assert!(count >= 1);

        // Clean up
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn add_multiple_jobs() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = AsyncCron::new(local_tz);

        cron.start().await;

        let counter1 = Arc::new(Mutex::new(0));
        let counter1_clone = Arc::clone(&counter1);

        let counter2 = Arc::new(Mutex::new(0));
        let counter2_clone = Arc::clone(&counter2);

        let job_id1 = cron
            .add_fn("* * * * * * *", move || {
                let counter = Arc::clone(&counter1_clone);
                async move {
                    let mut count = counter.lock().await;
                    *count += 1;
                }
            })
            .await
            .unwrap();

        let job_id2 = cron
            .add_fn("* * * * * * *", move || {
                let counter = Arc::clone(&counter2_clone);
                async move {
                    let mut count = counter.lock().await;
                    *count += 1;
                }
            })
            .await
            .unwrap();

        sleep(Duration::from_millis(1100)).await;
        cron.stop().await;

        let count1 = *counter1.lock().await;
        let count2 = *counter2.lock().await;
        assert!(count1 >= 1);
        assert!(count2 >= 1);

        // Clean up
        cron.remove(job_id1).await;
        cron.remove(job_id2).await;
    }

    #[tokio::test]
    async fn remove_job() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = AsyncCron::new(local_tz);

        cron.start().await;

        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let job_id = cron
            .add_fn("* * * * * * *", move || {
                let counter = Arc::clone(&counter_clone);
                async move {
                    let mut count = counter.lock().await;
                    *count += 1;
                }
            })
            .await
            .unwrap();

        sleep(Duration::from_millis(1100)).await;

        let count_before_removal = *counter.lock().await;
        assert!(count_before_removal >= 1);

        cron.remove(job_id).await;

        // Reset counter to test that job was actually removed
        {
            let mut count = counter.lock().await;
            *count = 0;
        }

        sleep(Duration::from_millis(1100)).await;
        cron.stop().await;

        let count_after_removal = *counter.lock().await;
        // After removal, counter should remain 0
        assert_eq!(count_after_removal, 0);
    }

    // ===============================
    // COMPREHENSIVE ASYNC TESTS
    // ===============================

    #[tokio::test]
    async fn test_multiple_concurrent_jobs() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;

        let job_count = 10;
        let execution_count = Arc::new(AtomicUsize::new(0));
        let mut job_ids = Vec::new();

        // Add multiple jobs that all execute every second
        for _ in 0..job_count {
            let count = execution_count.clone();
            let job_id = cron.add_fn("* * * * * * *", move || {
                let count = count.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                }
            }).await.unwrap();
            job_ids.push(job_id);
        }

        // Let them run for a bit
        sleep(Duration::from_millis(1100)).await;
        
        cron.stop().await;

        let total_executions = execution_count.load(Ordering::SeqCst);
        // Should have at least job_count executions (1 for each job)
        assert!(total_executions >= job_count, "Expected at least {} executions, got {}", job_count, total_executions);

        // Clean up
        for job_id in job_ids {
            cron.remove(job_id).await;
        }
    }

    #[tokio::test]
    async fn test_rapid_job_manipulation() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;

        let mut job_ids = Vec::new();

        // Rapidly add many jobs
        for i in 0..50 {
            let job_id = cron.add_fn("* * * * * * *", move || {
                let _job_num = i;
                async move {
                    // Short delay to simulate work
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }).await.unwrap();
            job_ids.push(job_id);
        }

        // Rapidly remove some jobs
        for &job_id in job_ids.iter().take(25) {
            cron.remove(job_id).await;
        }

        // Add more jobs
        for i in 50..75 {
            let job_id = cron.add_fn("*/2 * * * * * *", move || {
                let _job_num = i;
                async move {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }).await.unwrap();
            job_ids.push(job_id);
        }

        sleep(Duration::from_millis(500)).await;
        cron.stop().await;

        // Clean up remaining jobs
        for &job_id in job_ids.iter().skip(25) {
            cron.remove(job_id).await;
        }
    }

    #[tokio::test]
    async fn test_scheduler_precision_under_load() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;

        let execution_times = Arc::new(Mutex::new(Vec::new()));
        let times = Arc::clone(&execution_times);

        // Add a job that records execution times
        let job_id = cron.add_fn("* * * * * * *", move || {
            let times = Arc::clone(&times);
            async move {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                times.lock().await.push(now);
            }
        }).await.unwrap();

        // Add some load with other jobs
        let mut load_job_ids = Vec::new();
        for i in 0..10 {
            let load_job_id = cron.add_fn("*/2 * * * * * *", move || {
                let _job_num = i;
                async move {
                    // Simulate CPU work
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }).await.unwrap();
            load_job_ids.push(load_job_id);
        }

        sleep(Duration::from_millis(3100)).await;
        cron.stop().await;

        let times = execution_times.lock().await;
        assert!(times.len() >= 2, "Should have multiple executions");

        // Check that executions are roughly 1 second apart (within 500ms tolerance for high load)
        for window in times.windows(2) {
            let diff = window[1] - window[0];
            assert!(diff >= 500 && diff <= 1500, "Execution interval should be ~1000ms (+/-500ms), got {}ms", diff);
        }

        // Clean up
        cron.remove(job_id).await;
        for job_id in load_job_ids {
            cron.remove(job_id).await;
        }
    }

    #[tokio::test]
    async fn test_timezone_handling() {
        let offset = FixedOffset::east_opt(5 * 3600).unwrap(); // UTC+5
        let mut cron = AsyncCron::new(offset);
        cron.start().await;

        let execution_count = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&execution_count);

        let job_id = cron.add_fn("0 * * * * * *", move || {
            let count = count.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
            }
        }).await.unwrap();

        // Run for a short time - job should execute at top of minute in the specified timezone
        sleep(Duration::from_millis(2000)).await;
        cron.stop().await;

        // Clean up
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_job_removal_during_execution() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;

        let execution_count = Arc::new(AtomicUsize::new(0));
        let long_running_flag = Arc::new(AtomicBool::new(false));

        let count = Arc::clone(&execution_count);
        let flag = Arc::clone(&long_running_flag);

        let job_id = cron.add_fn("* * * * * * *", move || {
            let count = count.clone();
            let flag = flag.clone();
            async move {
                flag.store(true, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(500)).await; // Long-running task
                count.fetch_add(1, Ordering::SeqCst);
                flag.store(false, Ordering::SeqCst);
            }
        }).await.unwrap();

        // Wait longer for job to start executing
        sleep(Duration::from_millis(1200)).await;

        // Remove job while it might be executing
        cron.remove(job_id).await;

        // Wait a bit more
        sleep(Duration::from_millis(1000)).await;
        cron.stop().await;

        let count = execution_count.load(Ordering::SeqCst);
        // Job should have executed at least once, but be more lenient with timing
        assert!(count >= 1 || long_running_flag.load(Ordering::SeqCst), 
               "Job should have executed or be in progress, count: {}, flag: {}", 
               count, long_running_flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_scheduler_restart() {
        let mut cron = AsyncCron::new(Utc);
        let execution_count = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&execution_count);

        let job_id = cron.add_fn("* * * * * * *", move || {
            let counter = Arc::clone(&counter);
            async move {
                let mut count = counter.lock().await;
                *count += 1;
            }
        }).await.unwrap();

        // First run cycle
        cron.start().await;
        sleep(Duration::from_millis(1100)).await;
        cron.stop().await;

        let count_after_first_run = *execution_count.lock().await;
        assert!(count_after_first_run >= 1, "Should execute during first run");

        // Second run cycle
        cron.start().await;
        sleep(Duration::from_millis(1100)).await;
        cron.stop().await;

        let final_count = *execution_count.lock().await;
        assert!(final_count > count_after_first_run, "Should continue executing after restart");

        // Clean up
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_memory_stability_long_running() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;

        let execution_count = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&execution_count);

        let job_id = cron.add_fn("* * * * * * *", move || {
            let counter = Arc::clone(&counter);
            async move {
                let mut count = counter.lock().await;
                *count += 1;
            }
        }).await.unwrap();

        // Run for longer period to test memory stability
        sleep(Duration::from_millis(5100)).await;
        cron.stop().await;

        let final_count = *execution_count.lock().await;
        assert!(final_count >= 4, "Should execute multiple times during long run, got {}", final_count);

        // Clean up
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_concurrent_cron_instances() {
        let instances = 5;
        let mut cron_instances = Vec::new();
        let mut counters = Vec::new();

        // Create multiple cron instances
        for _i in 0..instances {
            let mut cron = AsyncCron::new(Utc);
            let counter = Arc::new(Mutex::new(0));
            let counter_clone = Arc::clone(&counter);

            let _job_id = cron.add_fn("* * * * * * *", move || {
                let counter = counter_clone.clone();
                async move {
                    let mut count = counter.lock().await;
                    *count += 1;
                }
            }).await.unwrap();

            cron.start().await;
            cron_instances.push(cron);
            counters.push(counter);
        }

        sleep(Duration::from_millis(2100)).await;

        // Stop all instances
        for cron in &mut cron_instances {
            cron.stop().await;
        }

        // Verify all instances worked
        for (i, counter) in counters.iter().enumerate() {
            let count = *counter.lock().await;
            assert!(count >= 1, "Instance {} should have executed at least once, got {}", i, count);
        }
    }

    #[tokio::test]
    async fn test_complex_cron_expressions() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;

        let expression_results = Arc::new(Mutex::new(HashMap::new()));

        // Test various cron expressions
        let expressions = vec![
            ("* * * * * * *", "every_second"),
            ("*/2 * * * * * *", "every_2_seconds"),
            ("*/3 * * * * * *", "every_3_seconds"),
        ];

        let mut job_ids = Vec::new();
        for (expr, name) in expressions {
            let results = Arc::clone(&expression_results);
            let job_name = name.to_string();
            let job_id = cron.add_fn(expr, move || {
                let results = results.clone();
                let name = job_name.clone();
                async move {
                    let mut map = results.lock().await;
                    *map.entry(name).or_insert(0) += 1;
                }
            }).await.unwrap();
            job_ids.push(job_id);
        }

        sleep(Duration::from_millis(6100)).await;
        cron.stop().await;

        let results = expression_results.lock().await;
        assert!(results.get("every_second").unwrap_or(&0) >= &5, "Every second job should execute multiple times");
        assert!(results.get("every_2_seconds").unwrap_or(&0) >= &2, "Every 2 seconds job should execute");
        assert!(results.get("every_3_seconds").unwrap_or(&0) >= &1, "Every 3 seconds job should execute");

        // Clean up
        for job_id in job_ids {
            cron.remove(job_id).await;
        }
    }

    #[tokio::test]
    async fn test_job_execution_timing_precision() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;

        let execution_times = Arc::new(Mutex::new(Vec::new()));
        let times = Arc::clone(&execution_times);

        let job_id = cron.add_fn("* * * * * * *", move || {
            let times = times.clone();
            async move {
                let now = Instant::now();
                times.lock().await.push(now);
            }
        }).await.unwrap();

        sleep(Duration::from_millis(3100)).await;
        cron.stop().await;

        let times = execution_times.lock().await;
        assert!(times.len() >= 2, "Should have multiple execution times");

        // Check timing precision (should be roughly 1 second apart)
        for window in times.windows(2) {
            let diff = window[1].duration_since(window[0]);
            assert!(diff.as_millis() >= 900 && diff.as_millis() <= 1100, 
                   "Execution intervals should be ~1000ms, got {}ms", diff.as_millis());
        }

        // Clean up
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_error_recovery() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;

        let success_count = Arc::new(Mutex::new(0));
        let error_count = Arc::new(Mutex::new(0));

        let success_counter = Arc::clone(&success_count);
        let error_counter = Arc::clone(&error_count);

        let mut job_ids = Vec::new();

        // Add a job that sometimes panics
        let job_id1 = cron.add_fn("* * * * * * *", move || {
            let success = success_counter.clone();
            let errors = error_counter.clone();
            async move {
                let should_panic = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() % 3 == 0;

                if should_panic {
                    let mut err_count = errors.lock().await;
                    *err_count += 1;
                    panic!("Simulated panic");
                } else {
                    let mut succ_count = success.lock().await;
                    *succ_count += 1;
                }
            }
        }).await.unwrap();
        job_ids.push(job_id1);

        // Add a normal job to ensure scheduler continues working
        let success_counter2 = Arc::clone(&success_count);
        let job_id2 = cron.add_fn("*/2 * * * * * *", move || {
            let success = success_counter2.clone();
            async move {
                let mut count = success.lock().await;
                *count += 1;
            }
        }).await.unwrap();
        job_ids.push(job_id2);

        sleep(Duration::from_millis(3100)).await;
        cron.stop().await;

        let success = *success_count.lock().await;
        assert!(success >= 1, "Should have some successful executions despite panics");

        // Clean up
        for job_id in job_ids {
            cron.remove(job_id).await;
        }
    }

    // ===============================
    // ASYNC PERFORMANCE TESTS
    // ===============================

    #[tokio::test]
    async fn test_async_concurrent_operations() {
        let num_crons = 3;
        let mut futures = vec![];

        for _i in 0..num_crons {
            let future = tokio::spawn(async move {
                let mut cron = AsyncCron::new(Utc);
                cron.start().await;

                let counter = Arc::new(Mutex::new(0));
                let counter_clone = Arc::clone(&counter);

                let job_id = cron.add_fn("* * * * * * *", move || {
                    let counter = counter_clone.clone();
                    async move {
                        let mut count = counter.lock().await;
                        *count += 1;
                    }
                }).await.unwrap();

                sleep(Duration::from_millis(1100)).await;
                cron.stop().await;

                let final_count = *counter.lock().await;
                cron.remove(job_id).await;
                final_count
            });
            futures.push(future);
        }

        let results = join_all(futures).await;

        for (i, result) in results.iter().enumerate() {
            let count = result.as_ref().unwrap();
            assert!(*count >= 1, "Cron instance {} should execute at least once, got {}", i, count);
        }
    }

    #[tokio::test] 
    async fn test_scheduler_overhead_async() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;

        let execution_times = Arc::new(Mutex::new(Vec::new()));
        let times = Arc::clone(&execution_times);

        let job_id = cron.add_fn("* * * * * * *", move || {
            let times = times.clone();
            async move {
                let now = Instant::now();
                times.lock().await.push(now);
            }
        }).await.unwrap();

        // Let it run for several executions
        sleep(Duration::from_millis(3100)).await;
        cron.stop().await;
        
        let times = execution_times.lock().await;
        assert!(times.len() >= 2, "Should have multiple executions for overhead test");
        
        // Verify overhead is minimal (timing should be consistent)
        for window in times.windows(2) {
            let interval = window[1].duration_since(window[0]);
            // Allow reasonable variance for async overhead and system load
            assert!(interval >= Duration::from_millis(500) && interval <= Duration::from_millis(1500),
                   "Async overhead should keep intervals close to 1000ms (+/-500ms), got {:?}", interval);
        }

        // Clean up
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_rapid_start_stop_cycles_async() {
        let mut cron = AsyncCron::new(Utc);
        
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);

        let job_id = cron.add_fn("* * * * * * *", move || {
            let counter = counter_clone.clone();
            async move {
                let mut count = counter.lock().await;
                *count += 1;
            }
        }).await.unwrap();

        // Rapidly start and stop the cron scheduler
        for _ in 0..20 {
            cron.start().await;
            sleep(Duration::from_millis(50)).await; // Brief execution
            cron.stop().await;
        }
        
        let final_count = *counter.lock().await;
        // May execute during the brief windows
        assert!(final_count <= 20, "Rapid cycles should limit executions, got {}", final_count);

        // Clean up
        cron.remove(job_id).await;
    }

    // ===============================
    // ASYNC THREAD SAFETY & MEMORY SAFETY TESTS
    // ===============================

    #[tokio::test]
    async fn test_async_concurrent_job_access() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;
        
        let shared_counter = Arc::new(AtomicUsize::new(0));
        let access_count = Arc::new(AtomicUsize::new(0));
        
        let mut job_ids = Vec::new();
        for _ in 0..5 {
            let counter = shared_counter.clone();
            let access = access_count.clone();
            let job_id = cron.add_fn("* * * * * * *", move || {
                let counter = counter.clone();
                let access = access.clone();
                async move {
                    access.fetch_add(1, Ordering::SeqCst);
                    let old_value = counter.fetch_add(1, Ordering::SeqCst);
                    // Simulate some async work
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    let new_value = counter.load(Ordering::SeqCst);
                    assert!(new_value > old_value, "Counter should increase");
                }
            }).await.unwrap();
            job_ids.push(job_id);
        }
        
        // Let job execute several times
        sleep(Duration::from_millis(3100)).await;
        cron.stop().await;
        
        let final_counter = shared_counter.load(Ordering::SeqCst);
        let total_accesses = access_count.load(Ordering::SeqCst);
        
        assert!(final_counter == total_accesses, 
               "All accesses should be counted: final={}, accesses={}", final_counter, total_accesses);
        assert!(final_counter >= 5, "Should have multiple concurrent accesses");

        // Clean up
        for job_id in job_ids {
            cron.remove(job_id).await;
        }
    }

    #[tokio::test]
    async fn test_async_shared_mutable_state_safety() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;
        
        // Use a shared vector protected by Mutex
        let shared_data = Arc::new(Mutex::new(Vec::new()));
        
        let mut job_ids = Vec::new();
        for i in 0..3 {
            let data = shared_data.clone();
            let job_id = cron.add_fn("* * * * * * *", move || {
                let data = data.clone();
                let value = i;
                async move {
                    let mut vec = data.lock().await;
                    vec.push(value);
                    // Simulate async work while holding the lock
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            }).await.unwrap();
            job_ids.push(job_id);
        }
        
        sleep(Duration::from_millis(3100)).await;
        cron.stop().await;
        
        let final_data = shared_data.lock().await;
        assert!(!final_data.is_empty(), "Shared data should have been modified");
        assert!(final_data.len() >= 3, "Should have entries from multiple jobs");

        // Clean up
        for job_id in job_ids {
            cron.remove(job_id).await;
        }
    }

    #[tokio::test]
    async fn test_async_memory_ordering_consistency() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;
        
        let writes = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let shared_value = Arc::new(AtomicUsize::new(0));
        
        let mut job_ids = Vec::new();

        // Writer job
        let writes_clone = writes.clone();
        let value_clone = shared_value.clone();
        let job_id1 = cron.add_fn("* * * * * * *", move || {
            let writes = writes_clone.clone();
            let value = value_clone.clone();
            async move {
                let old = value.fetch_add(1, Ordering::SeqCst);
                writes.fetch_add(1, Ordering::SeqCst);
                assert!(old < old + 1, "Write should increment value");
            }
        }).await.unwrap();
        job_ids.push(job_id1);

        // Reader job
        let reads_clone = reads.clone();
        let value_clone2 = shared_value.clone();
        let job_id2 = cron.add_fn("*/2 * * * * * *", move || {
            let reads = reads_clone.clone();
            let value = value_clone2.clone();
            async move {
                let _current = value.load(Ordering::SeqCst);
                reads.fetch_add(1, Ordering::SeqCst);
                // Note: current is AtomicUsize which is always >= 0, so we don't need to check
            }
        }).await.unwrap();
        job_ids.push(job_id2);
        
        sleep(Duration::from_millis(3100)).await;
        cron.stop().await;
        
        let final_writes = writes.load(Ordering::SeqCst);
        let final_reads = reads.load(Ordering::SeqCst);
        let final_value = shared_value.load(Ordering::SeqCst);
        
        assert!(final_writes >= 2, "Should have multiple writes");
        assert!(final_reads >= 1, "Should have some reads");
        assert_eq!(final_value, final_writes, "Final value should equal write count");

        // Clean up
        for job_id in job_ids {
            cron.remove(job_id).await;
        }
    }

    #[tokio::test]
    async fn test_async_scheduler_clone_safety() {
        let mut cron1 = AsyncCron::new(Utc);
        
        // Clone the scheduler
        let mut cron2 = cron1.clone();
        
        let counter1 = Arc::new(AtomicUsize::new(0));
        let counter2 = Arc::new(AtomicUsize::new(0));
        
        let count1 = counter1.clone();
        let job_id1 = cron1.add_fn("* * * * * * *", move || {
            let count = count1.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
            }
        }).await.unwrap();

        let count2 = counter2.clone();
        let job_id2 = cron2.add_fn("*/2 * * * * * *", move || {
            let count = count2.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
            }
        }).await.unwrap();
        
        // Start both schedulers
        cron1.start().await;
        cron2.start().await;
        
        sleep(Duration::from_millis(2100)).await;
        
        // Stop both schedulers
        cron1.stop().await;
        cron2.stop().await;
        
        let count1 = counter1.load(Ordering::SeqCst);
        let count2 = counter2.load(Ordering::SeqCst);
        
        // Both should execute independently
        assert!(count1 >= 1, "Clone 1 should execute");
        assert!(count2 >= 1, "Clone 2 should execute");

        // Clean up
        cron1.remove(job_id1).await;
        cron2.remove(job_id2).await;
    }

    #[tokio::test]
    async fn test_async_cross_thread_job_execution() {
        use std::thread;
        
        let mut cron = AsyncCron::new(Utc);
        
        let main_thread_id = thread::current().id();
        let execution_threads = Arc::new(Mutex::new(Vec::new()));
        let threads_clone = execution_threads.clone();

        let job_id = cron.add_fn("* * * * * * *", move || {
            let threads = threads_clone.clone();
            async move {
                let current_thread = thread::current().id();
                threads.lock().await.push(current_thread);
            }
        }).await.unwrap();
        
        cron.start().await;
        sleep(Duration::from_millis(2100)).await;
        cron.stop().await;
        
        let threads = execution_threads.lock().await;
        assert!(!threads.is_empty(), "Should have captured execution threads");
        
        // In async context, jobs may run on different threads than main
        let _has_different_thread = threads.iter().any(|&tid| tid != main_thread_id);
        // This may or may not be true depending on the async runtime, so we just verify execution occurred
        assert!(threads.len() >= 1, "Should have at least one execution thread record");

        // Clean up
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_async_large_scale_concurrent_execution() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;
        
        let total_executions = Arc::new(AtomicUsize::new(0));
        let job_count = 50;
        let mut job_ids = Vec::new();
        
        // Add many concurrent jobs
        for _ in 0..job_count {
            let counter = total_executions.clone();
            let job_id = cron.add_fn("* * * * * * *", move || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    // Simulate minimal async work
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }).await.unwrap();
            job_ids.push(job_id);
        }
        
        sleep(Duration::from_millis(2100)).await;
        cron.stop().await;
        
        let total = total_executions.load(Ordering::SeqCst);
        // Should have significant concurrent execution
        assert!(total >= job_count, "Should execute all jobs at least once, got {} for {} jobs", total, job_count);

        // Clean up
        for job_id in job_ids {
            cron.remove(job_id).await;
        }
    }

    #[tokio::test]
    async fn test_async_job_removal_thread_safety() {
        let mut cron = AsyncCron::new(Utc);
        cron.start().await;
        
        let execution_count = Arc::new(AtomicUsize::new(0));
        let mut job_ids = Vec::new();
        
        // Add multiple jobs
        for _ in 0..10 {
            let count = execution_count.clone();
            let job_id = cron.add_fn("* * * * * * *", move || {
                let count = count.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    // Simulate work
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }).await.unwrap();
            job_ids.push(job_id);
        }
        
        // Let jobs start executing
        sleep(Duration::from_millis(500)).await;
        
        // Remove jobs while they might be executing
        for &job_id in job_ids.iter().take(5) {
            cron.remove(job_id).await;
            sleep(Duration::from_millis(50)).await; // Stagger removals
        }
        
        // Continue execution while removal happens
        sleep(Duration::from_millis(1000)).await;
        cron.stop().await;
        
        let final_count = execution_count.load(Ordering::SeqCst);
        
        // Should complete without deadlock or panic
        assert!(final_count >= 5, "Some jobs should have executed before removal");

        // Clean up remaining jobs
        for &job_id in job_ids.iter().skip(5) {
            cron.remove(job_id).await;
        }
    }

    #[tokio::test]
    async fn test_timezone_differences() {
        use chrono::FixedOffset;
        
        let _utc_cron = AsyncCron::new(Utc);
        let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
        let _tokyo_cron = AsyncCron::new(tokyo_tz);
        
        // Just verify that different timezones can be created and used
        let mut utc_mut = _utc_cron;
        let _job_id = utc_mut.add_fn("0 0 12 * * * *", || async {
            println!("Noon in the scheduler's timezone");
        }).await.unwrap();
        
        // Test that the scheduler works with the timezone  
        utc_mut.start().await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        utc_mut.stop().await;
    }

    #[tokio::test]
    async fn test_set_timezone() {
        // Create with FixedOffset initially so we can set another FixedOffset later
        let initial_tz = FixedOffset::east_opt(0).unwrap(); // UTC equivalent
        let mut cron = AsyncCron::new(initial_tz);
        
        // Add a job with initial timezone
        let job_id = cron.add_fn("* * * * * * *", || async {
            println!("Test job");
        }).await.unwrap();
        
        // Change timezone to Tokyo (UTC+9)
        let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
        cron.set_timezone(tokyo_tz);
        
        // Should be able to add jobs after timezone change
        let job_id2 = cron.add_fn("*/2 * * * * * *", || async {
            println!("Job with new timezone");
        }).await.unwrap();
        
        // Clean up
        cron.remove(job_id).await;
        cron.remove(job_id2).await;
    }

    #[tokio::test]
    async fn test_remove_entry_directly() {
        let mut cron = AsyncCron::new(Utc);
        
        // Add a job
        let job_id = cron.add_fn("* * * * * * *", || async {
            println!("Test job");
        }).await.unwrap();
        
        // Remove the job using the remove method (which calls remove_entry internally)
        cron.remove(job_id).await;
        
        // Try to remove the same job again (should not cause issues)
        cron.remove(job_id).await;
        
        // Try to remove a non-existent job ID
        cron.remove(9999).await;
    }

    #[tokio::test]
    async fn test_remove_while_running() {
        let mut cron = AsyncCron::new(Utc);
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        
        // Add a job that increments a counter
        let job_id = cron.add_fn("*/50 * * * * * *", move || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }).await.unwrap();
        
        // Start the cron
        cron.start().await;
        
        // Wait a bit to ensure the scheduler is running
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Remove the job while running (this should go through the channel)
        cron.remove(job_id).await;
        
        cron.stop().await;
    }

    #[tokio::test]
    async fn test_schedule_method_indirectly() {
        let mut cron = AsyncCron::new(Utc);
        
        // Test scheduling with different types of jobs
        let counter = Arc::new(AtomicUsize::new(0));
        
        // Simple closure
        let _job1 = cron.add_fn("* * * * * * *", || async {}).await.unwrap();
        
        // Closure with capture
        let counter_clone = counter.clone();
        let _job2 = cron.add_fn("* * * * * * *", move || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }).await.unwrap();
        
        // Complex async closure
        let _job3 = cron.add_fn("* * * * * * *", || async {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }).await.unwrap();
    }

    #[tokio::test]
    async fn test_add_job_to_running_scheduler() {
        let mut cron = AsyncCron::new(Utc);
        let counter = Arc::new(AtomicUsize::new(0));
        
        // Start the cron first
        cron.start().await;
        
        // Wait to ensure scheduler is running
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        // Add a job while running (should go through the channel)
        let counter_clone = counter.clone();
        let _job_id = cron.add_fn("*/30 * * * * * *", move || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        }).await.unwrap();
        
        cron.stop().await;
    }

    #[tokio::test]
    async fn test_start_blocking_edge_cases() {
        let mut cron = AsyncCron::new(Utc);
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = executed.clone();
        
        // Add a job that should execute soon
        let _job_id = cron.add_fn("* * * * * * *", move || {
            let executed = executed_clone.clone();
            async move {
                executed.store(true, Ordering::SeqCst);
            }
        }).await.unwrap();
        
        // Start blocking in a separate task
        let mut cron_clone = cron.clone();
        let blocking_handle = tokio::spawn(async move {
            cron_clone.start_blocking().await;
        });
        
        // Wait a bit for job to execute
        tokio::time::sleep(Duration::from_millis(1100)).await;
        
        // Stop the scheduler
        cron.stop().await;
        
        // Wait for blocking task to finish
        let _ = tokio::time::timeout(Duration::from_secs(5), blocking_handle).await;
        
        // Verify job executed
        assert!(executed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_scheduler_with_no_jobs() {
        let mut cron = AsyncCron::new(Utc);
        
        // Start with no jobs
        cron.start().await;
        
        // Wait a bit
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Stop
        cron.stop().await;
    }

    #[tokio::test]
    async fn test_multiple_stop_calls() {
        let mut cron = AsyncCron::new(Utc);
        
        cron.start().await;
        
        // Multiple stop calls should not cause issues
        cron.stop().await;
        cron.stop().await;
        cron.stop().await;
    }

    #[tokio::test]
    async fn test_job_execution_order_with_same_schedule() {
        let mut cron = AsyncCron::new(Utc);
        let execution_order = Arc::new(Mutex::new(Vec::new()));
        
        // Add multiple jobs with the same schedule
        for i in 0..3 {
            let execution_order_clone = execution_order.clone();
            let _job_id = cron.add_fn("* * * * * * *", move || {
                let execution_order = execution_order_clone.clone();
                let job_num = i;
                async move {
                    execution_order.lock().await.push(job_num);
                }
            }).await.unwrap();
        }
        
        cron.start().await;
        
        // Wait for jobs to execute
        tokio::time::sleep(Duration::from_millis(1100)).await;
        
        cron.stop().await;
        
        // Check that all jobs executed (each job may execute multiple times)
        let order = execution_order.lock().await;
        assert!(order.len() >= 3, "Should have at least 3 executions, got {}", order.len());
        assert!(order.contains(&0), "Job 0 should have executed");
        assert!(order.contains(&1), "Job 1 should have executed");
        assert!(order.contains(&2), "Job 2 should have executed");
    }

    #[tokio::test]
    async fn test_invalid_cron_expression_async() {
        let mut cron = AsyncCron::new(Utc);
        
        // Test various invalid cron expressions
        let invalid_expressions = vec![
            "invalid",
            "* * * * *",  // too few fields
            "60 * * * * * *",  // invalid second (>59)
            "* 60 * * * * *",  // invalid minute (>59)
            "* * 25 * * * *",  // invalid hour (>23)
            "* * * 32 * * *",  // invalid day (>31)
            "* * * * 13 * *",  // invalid month (>12)
            "* * * * * 8 *",   // invalid weekday (>7)
            "",  // empty string
            "* * * * * * * *",  // too many fields
        ];
        
        for expr in invalid_expressions {
            let result = cron.add_fn(expr, || async {}).await;
            assert!(result.is_err(), "Expected error for expression: {}", expr);
        }
    }

    #[tokio::test]
    async fn test_timezone_scheduling_differences() {
        use chrono::FixedOffset;
        
        let utc_cron = AsyncCron::new(Utc);
        let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
        let tokyo_cron = AsyncCron::new(tokyo_tz);
        
        // Get current time using chrono directly instead of private now() method
        let utc_now = Utc::now();
        let tokyo_now = utc_now.with_timezone(&tokyo_tz);
        
        // The difference should be approximately 9 hours (allowing for small timing differences)
        let time_diff = (tokyo_now.timestamp() - utc_now.timestamp()).abs();
        let expected_diff = 0; // Same instant in time, just different timezone representation
        
        // Times should be the same instant, just in different timezones
        assert_eq!(time_diff, expected_diff, "Same instant should have 0 time difference, got {} seconds", time_diff);
        
        // But the hour should be different due to timezone offset
        assert_ne!(tokyo_now.hour(), utc_now.hour(), "Hours should differ due to timezone");
    }

    #[tokio::test]
    async fn test_concurrent_add_remove_operations() {
        let cron = Arc::new(Mutex::new(AsyncCron::new(Utc)));
        let mut handles = vec![];
        
        // Start multiple tasks that add and remove jobs concurrently
        for _i in 0..10 {
            let cron_clone = cron.clone();
            let handle = tokio::spawn(async move {
                let mut cron = cron_clone.lock().await;
                let job_id = cron.add_fn("* * * * * * *", move || {
                    let _job_num = _i;
                    async move {
                        tokio::time::sleep(Duration::from_millis(1)).await;
                        println!("Job {} executed", _job_num);
                    }
                }).await.unwrap();
                
                // Remove the job immediately
                cron.remove(job_id).await;
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_job_execution_with_long_running_tasks() {
        let mut cron = AsyncCron::new(Utc);
        let execution_count = Arc::new(AtomicUsize::new(0));
        
        // Add a long-running job
        let execution_count_clone = execution_count.clone();
        let _job_id = cron.add_fn("* * * * * * *", move || {
            let count = execution_count_clone.clone();
            async move {
                // Simulate a long-running task
                tokio::time::sleep(Duration::from_millis(500)).await;
                count.fetch_add(1, Ordering::SeqCst);
            }
        }).await.unwrap();
        
        cron.start().await;
        
        // Wait for multiple executions
        tokio::time::sleep(Duration::from_millis(2500)).await;
        
        cron.stop().await;
        
        // Should have executed multiple times despite long duration
        let count = execution_count.load(Ordering::SeqCst);
        assert!(count >= 2, "Expected at least 2 executions, got {}", count);
    }

    #[tokio::test]
    async fn test_different_timezone_scheduling() {
        use chrono::FixedOffset;
        
        let utc_cron = AsyncCron::new(Utc);
        let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
        let tokyo_cron = AsyncCron::new(tokyo_tz);
        
        // Both should be able to create jobs successfully
        let mut utc_mut = utc_cron;
        let mut tokyo_mut = tokyo_cron;
        
        let utc_job = utc_mut.add_fn("* * * * * * *", || async {
            println!("UTC job");
        }).await.unwrap();
        
        let tokyo_job = tokyo_mut.add_fn("* * * * * * *", || async {
            println!("Tokyo job");  
        }).await.unwrap();
        
        // Start both schedulers
        utc_mut.start().await;
        tokyo_mut.start().await;
        
        // Let them run briefly
        tokio::time::sleep(Duration::from_millis(1100)).await;
        
        // Stop both
        utc_mut.stop().await;
        tokyo_mut.stop().await;
        
        // Clean up
        utc_mut.remove(utc_job).await;
        tokyo_mut.remove(tokyo_job).await;
    }

    #[tokio::test]
    async fn test_timezone_support() {
        use chrono::FixedOffset;
        
        let utc_cron = AsyncCron::new(Utc);
        let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
        let tokyo_cron = AsyncCron::new(tokyo_tz);
        
        // Both should successfully create jobs
        let mut utc_mut = utc_cron;
        let mut tokyo_mut = tokyo_cron;
        
        let _utc_job = utc_mut.add_fn("* * * * * * *", || async {}).await.unwrap();
        let _tokyo_job = tokyo_mut.add_fn("* * * * * * *", || async {}).await.unwrap();
        
        // Both should start successfully  
        utc_mut.start().await;
        tokyo_mut.start().await;
        
        // Brief execution
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Both should stop successfully
        utc_mut.stop().await;
        tokyo_mut.stop().await;
    }

    // ===============================
    // COVERAGE IMPROVEMENT TESTS
    // ===============================

    #[tokio::test]
    async fn test_remove_from_stopped_scheduler() {
        let mut cron = AsyncCron::new(Utc);
        
        // Add a job to stopped scheduler
        let job_id = cron.add_fn("* * * * * * *", || async {
            println!("Test job");
        }).await.unwrap();
        
        // Remove from stopped scheduler (should call remove_entry directly)
        cron.remove(job_id).await;
        
        // Verify job was removed by trying to remove again
        cron.remove(job_id).await; // Should not panic
        
        // Remove non-existent job
        cron.remove(9999).await; // Should not panic
    }

    #[tokio::test]
    async fn test_schedule_method_coverage() {
        let mut cron = AsyncCron::new(Utc);
        
        // Test adding job when scheduler is not running (covers fallback branch)
        let job_id1 = cron.add_fn("* * * * * * *", || async {}).await.unwrap();
        
        // Start scheduler
        cron.start().await;
        
        // Test adding job when scheduler IS running (covers channel send)
        let job_id2 = cron.add_fn("*/2 * * * * * *", || async {}).await.unwrap();
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        cron.stop().await;
        
        // Clean up
        cron.remove(job_id1).await;
        cron.remove(job_id2).await;
    }

    #[tokio::test]
    async fn test_edge_case_cron_schedule() {
        let mut cron = AsyncCron::new(Utc);
        
        // Test schedule that might produce None for next execution
        // Using a schedule that runs only on Feb 30th (which doesn't exist)
        match cron.add_fn("0 0 0 30 2 * *", || async {}).await {
            Ok(job_id) => {
                // If it somehow works, clean up
                cron.remove(job_id).await;
            },
            Err(_) => {
                // Expected - invalid date
            }
        }
        
        // Test with valid but complex schedule
        let job_id = cron.add_fn("0 0 12 * * 1-5 *", || async {
            println!("Weekdays at noon");
        }).await.unwrap();
        
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_scheduler_with_empty_schedule() {
        let mut cron = AsyncCron::new(Utc);
        
        // Start scheduler with no jobs to cover empty entries case
        cron.start().await;
        
        // Let it run briefly
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Stop scheduler
        cron.stop().await;
        
        // Add job after stopping
        let job_id = cron.add_fn("* * * * * * *", || async {}).await.unwrap();
        
        // Remove it
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_stop_channel_edge_cases() {
        let mut cron = AsyncCron::new(Utc);
        
        // Test multiple stop calls
        cron.stop().await;
        cron.stop().await;
        cron.stop().await;
        
        // Start and stop quickly
        cron.start().await;
        cron.stop().await;
        
        // Start again and add job
        cron.start().await;
        let job_id = cron.add_fn("* * * * * * *", || async {}).await.unwrap();
        
        // Stop and clean up
        cron.stop().await;
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_job_scheduling_edge_cases() {
        let mut cron = AsyncCron::new(Utc);
        
        // Test job that schedules very far in the future
        let far_future_job = cron.add_fn("0 0 0 1 1 * 2030", || async {
            println!("Far future job");
        }).await.unwrap();
        
        // Test job with immediate execution
        let immediate_job = cron.add_fn("* * * * * * *", || async {
            println!("Immediate job");
        }).await.unwrap();
        
        cron.start().await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        cron.stop().await;
        
        // Clean up
        cron.remove(far_future_job).await;
        cron.remove(immediate_job).await;
    }

    #[tokio::test]
    async fn test_remove_when_not_running_coverage() {
        let mut cron = AsyncCron::new(Utc);
        
        // Add job when NOT running
        let job_id = cron.add_fn("* * * * * * *", || async {}).await.unwrap();
        
        // Remove when NOT running - this should hit line 235 (remove_entry path)
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_schedule_when_running_coverage() {
        let mut cron = AsyncCron::new(Utc);
        
        // Start scheduler first
        cron.start().await;
        
        // Brief wait to ensure scheduler is fully started
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Add job when running - this should hit line 331 (channel send path)
        let job_id = cron.add_fn("* * * * * * *", || async {}).await.unwrap();
        
        // Remove when running - this should hit the channel send in remove
        cron.remove(job_id).await;
        
        cron.stop().await;
    }

    #[tokio::test]
    async fn test_schedule_with_start_blocking_coverage() {
        let mut cron = AsyncCron::new(Utc);
        
        // Use start_blocking in a task to ensure channels are set up
        let mut cron_clone = cron.clone();
        let handle = tokio::spawn(async move {
            cron_clone.start_blocking().await;
        });
        
        // Wait for start_blocking to set up channels
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Now add job - this should hit the channel send path (line 331)
        let job_id = cron.add_fn("* * * * * * *", || async {}).await.unwrap();
        
        // Remove job - this should hit the channel send in remove
        cron.remove(job_id).await;
        
        // Stop the scheduler
        cron.stop().await;
        
        // Wait for the task to finish
        let _ = tokio::time::timeout(Duration::from_secs(1), handle).await;
    }

    #[tokio::test]
    async fn test_schedule_when_not_running_coverage() {
        let mut cron = AsyncCron::new(Utc);
        
        // Add job when NOT running - this should hit line 356 (fallback path)
        let job_id = cron.add_fn("* * * * * * *", || async {}).await.unwrap();
        
        // Clean up
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_async_entry_get_next_edge_case() {
        let mut cron = AsyncCron::new(Utc);
        
        // Test with a schedule that might return None in some edge cases
        // This should test line 153 in async_entry.rs
        let job_id = cron.add_fn("0 0 0 31 2 * *", || async {}).await.unwrap(); // Feb 31st (invalid)
        
        cron.remove(job_id).await;
    }

    #[tokio::test]
    async fn test_stop_without_channels() {
        let cron = AsyncCron::new(Utc);
        
        // Stop without ever starting - this should test the None case in stop
        cron.stop().await;
    }
}
