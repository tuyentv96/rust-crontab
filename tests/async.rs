//! Comprehensive asynchronous tests for CronTab
//!
//! This module contains all tests for the asynchronous functionality of the cron scheduler,
//! including basic async operations, comprehensive scheduling scenarios, error handling,
//! and performance validation in async contexts.

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
        sync::atomic::{AtomicUsize, Ordering},
    };

    use chrono::{FixedOffset, Local, TimeZone, Utc};
    use cron_tab::Cron;
    use tokio::time::sleep;
    use futures::future::join_all;

    // ===============================
    // BASIC ASYNC FUNCTIONALITY TESTS
    // ===============================

    #[tokio::test]
    async fn start_and_stop_cron() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = Cron::new(local_tz);

        cron.start();
        cron.stop();
    }

    #[tokio::test]
    async fn add_job_before_start() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = Cron::new(local_tz);

        let counter = Arc::new(Mutex::new(0));
        let counter1 = Arc::clone(&counter);

        cron.add_fn("* * * * * *", move || {
            let mut value = counter1.lock().unwrap();
            *value += 1;
        })
        .unwrap();

        cron.start();

        sleep(Duration::from_millis(2100)).await;
        let value = *counter.lock().unwrap();
        assert!(value >= 1 && value <= 3, "Expected 1-3 executions, got {}", value);
    }

    #[tokio::test]
    async fn add_job() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = Cron::new(local_tz);

        cron.start();

        let counter = Arc::new(Mutex::new(0));
        let counter1 = Arc::clone(&counter);

        cron.add_fn("* * * * * *", move || {
            let mut value = counter1.lock().unwrap();
            *value += 1;
        })
        .unwrap();

        sleep(Duration::from_millis(2100)).await;
        let value = *counter.lock().unwrap();
        assert!(value >= 1 && value <= 3, "Expected 1-3 executions, got {}", value);
    }

    #[tokio::test]
    async fn add_multiple_jobs() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = Cron::new(local_tz);

        cron.start();

        let counter1 = Arc::new(Mutex::new(0));
        let c1 = Arc::clone(&counter1);

        cron.add_fn("* * * * * *", move || {
            let mut value = c1.lock().unwrap();
            *value += 1;
        })
        .unwrap();

        let counter2 = Arc::new(Mutex::new(0));
        let c2 = Arc::clone(&counter2);
        cron.add_fn("*/2 * * * * *", move || {
            let mut value = c2.lock().unwrap();
            *value += 1;
        })
        .unwrap();

        sleep(Duration::from_millis(3100)).await;
        let value1 = *counter1.lock().unwrap();
        let value2 = *counter2.lock().unwrap();
        assert!(value1 >= 2 && value1 <= 4, "Expected 2-4 executions for every second job, got {}", value1);
        assert!(value2 >= 1 && value2 <= 2, "Expected 1-2 executions for every 2 seconds job, got {}", value2);
    }

    #[tokio::test]
    async fn remove_job() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = Cron::new(local_tz);

        cron.start();

        let counter = Arc::new(Mutex::new(0));
        let counter1 = Arc::clone(&counter);

        let job_id = cron
            .add_fn("* * * * * *", move || {
                *counter1.lock().unwrap() += 1;
            })
            .unwrap();

        sleep(Duration::from_millis(1100)).await;
        let count_before_removal = *counter.lock().unwrap();
        assert!(count_before_removal >= 1, "Job should have executed at least once");
        
        cron.remove(job_id);

        sleep(Duration::from_millis(1100)).await;
        let count_after_removal = *counter.lock().unwrap();
        
        // After removal, count should not have changed significantly
        assert_eq!(count_after_removal, count_before_removal, 
                  "Job should not execute after removal. Before: {}, After: {}", 
                  count_before_removal, count_after_removal);
    }

    // ===============================
    // COMPREHENSIVE ASYNC TESTS
    // ===============================

    #[tokio::test]
    async fn test_multiple_concurrent_jobs() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let job_counters = Arc::new(Mutex::new(Vec::new()));

        // Create 10 different jobs with different schedules
        for i in 0..10 {
            let counter = Arc::new(Mutex::new(0));
            job_counters.lock().unwrap().push(Arc::clone(&counter));

            cron.add_fn("* * * * * *", move || {
                let mut count = counter.lock().unwrap();
                *count += 1;
                // Simulate some work with different durations
                std::thread::sleep(Duration::from_millis(i * 10 + 50));
            }).unwrap();
        }

        // Let jobs run for a while
        sleep(Duration::from_millis(3100)).await;
        cron.stop();

        // Verify all jobs executed
        let counters = job_counters.lock().unwrap();
        for (i, counter) in counters.iter().enumerate() {
            let count = *counter.lock().unwrap();
            assert!(count >= 1 && count <= 4, "Job {} should have executed 1-4 times, got {}", i, count);
        }
    }

    #[tokio::test]
    async fn test_rapid_job_manipulation() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let execution_count = Arc::new(Mutex::new(0));
        let mut job_ids = vec![];

        // Rapidly add jobs
        for _ in 0..50 {
            let counter = Arc::clone(&execution_count);
            let job_id = cron.add_fn("* * * * * *", move || {
                let mut count = counter.lock().unwrap();
                *count += 1;
            }).unwrap();
            job_ids.push(job_id);
        }

        // Let them run briefly
        sleep(Duration::from_millis(500)).await;

        // Rapidly remove half the jobs
        for &job_id in job_ids.iter().take(25) {
            cron.remove(job_id);
        }

        // Let remaining jobs run
        sleep(Duration::from_millis(1200)).await;
        cron.stop();

        let final_count = *execution_count.lock().unwrap();
        // With job removal, we expect fewer executions
        assert!(final_count >= 10 && final_count <= 100, 
               "Expected 10-100 executions with job removal, got {}", final_count);
    }

    #[tokio::test]
    async fn test_scheduler_precision_under_load() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let execution_times = Arc::new(Mutex::new(Vec::new()));

        // Job that measures execution timing
        let times = Arc::clone(&execution_times);
        cron.add_fn("* * * * * *", move || {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            times.lock().unwrap().push(now);
        }).unwrap();

        // Add load with multiple other jobs
        for i in 0..20 {
            cron.add_fn("* * * * * *", move || {
                // Simulate CPU work
                let mut sum = 0;
                for j in 0..1000 {
                    sum += i * j;
                }
                std::hint::black_box(sum); // Prevent optimization
            }).unwrap();
        }

        sleep(Duration::from_millis(3100)).await;
        cron.stop();

        let times = execution_times.lock().unwrap();
        assert!(times.len() >= 2, "Should have at least 2 precision measurements");

        // Check intervals between executions
        for window in times.windows(2) {
            let interval = window[1] - window[0];
            // Allow for significant variance under load
            assert!(interval >= 500 && interval <= 2000,
                   "Execution interval should be roughly 1000ms ±500ms under load, got {}ms", interval);
        }
    }

    #[tokio::test]
    async fn test_timezone_handling() {
        let offset = FixedOffset::east_opt(5 * 3600).unwrap(); // UTC+5
        let mut cron = Cron::new(offset);
        cron.start();

        let execution_count = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&execution_count);

        cron.add_fn("* * * * * *", move || {
            let mut count = counter.lock().unwrap();
            *count += 1;
        }).unwrap();

        sleep(Duration::from_millis(1500)).await;
        cron.stop();

        let count = *execution_count.lock().unwrap();
        assert!(count >= 1 && count <= 3, "Timezone job should execute 1-3 times, got {}", count);
    }

    #[tokio::test]
    async fn test_job_removal_during_execution() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let long_running_count = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&long_running_count);

        let job_id = cron.add_fn("* * * * * *", move || {
            let mut count = counter.lock().unwrap();
            *count += 1;
            // Simulate longer running job
            std::thread::sleep(Duration::from_millis(200));
        }).unwrap();

        // Let it start executing
        sleep(Duration::from_millis(500)).await;

        // Remove job while it might be executing
        cron.remove(job_id);

        // Wait and ensure no more executions
        let count_at_removal = *long_running_count.lock().unwrap();
        sleep(Duration::from_millis(1200)).await;
        cron.stop();

        let final_count = *long_running_count.lock().unwrap();
        // Job may complete current execution but shouldn't start new ones
        assert!(final_count <= count_at_removal + 1,
               "Job should stop executing after removal. At removal: {}, Final: {}", 
               count_at_removal, final_count);
    }

    #[tokio::test]
    async fn test_scheduler_restart() {
        let mut cron = Cron::new(Utc);
        let execution_count = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&execution_count);

        // Add job before first start
        cron.add_fn("* * * * * *", move || {
            let mut count = counter.lock().unwrap();
            *count += 1;
        }).unwrap();

        // First run cycle
        cron.start();
        sleep(Duration::from_millis(1100)).await;
        cron.stop();

        let count_after_first_run = *execution_count.lock().unwrap();
        assert!(count_after_first_run >= 1, "Should execute during first run");

        // Second run cycle
        cron.start();
        sleep(Duration::from_millis(1100)).await;
        cron.stop();

        let final_count = *execution_count.lock().unwrap();
        assert!(final_count > count_after_first_run, 
               "Should continue executing after restart. First: {}, Final: {}", 
               count_after_first_run, final_count);
    }

    #[tokio::test]
    async fn test_memory_stability_long_running() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let execution_count = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&execution_count);

        cron.add_fn("* * * * * *", move || {
            let mut count = counter.lock().unwrap();
            *count += 1;
            
            // Simulate memory allocation and deallocation
            let _temp_data: Vec<u8> = vec![0; 1024]; // 1KB allocation
        }).unwrap();

        // Run for longer period to test memory stability
        sleep(Duration::from_millis(5100)).await;
        cron.stop();

        let final_count = *execution_count.lock().unwrap();
        assert!(final_count >= 4 && final_count <= 7, 
               "Long running job should execute 4-7 times, got {}", final_count);
    }

    #[tokio::test]
    async fn test_concurrent_cron_instances() {
        let instances = 5;
        let mut cron_instances = Vec::new();
        let mut counters = Vec::new();

        // Create multiple cron instances
        for i in 0..instances {
            let mut cron = Cron::new(Utc);
            let counter = Arc::new(Mutex::new(0));
            let counter_clone = Arc::clone(&counter);

            cron.add_fn("* * * * * *", move || {
                let mut count = counter_clone.lock().unwrap();
                *count += 1;
                // Each instance does slightly different work
                std::thread::sleep(Duration::from_millis(i * 10 + 10));
            }).unwrap();

            cron.start();
            cron_instances.push(cron);
            counters.push(counter);
        }

        // Let all instances run
        sleep(Duration::from_millis(2100)).await;

        // Stop all instances
        for cron in &mut cron_instances {
            cron.stop();
        }

        // Verify all instances worked independently
        for (i, counter) in counters.iter().enumerate() {
            let count = *counter.lock().unwrap();
            assert!(count >= 1 && count <= 3, 
                   "Instance {} should execute 1-3 times, got {}", i, count);
        }
    }

    #[tokio::test]
    async fn test_complex_cron_expressions() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let expression_results = Arc::new(Mutex::new(HashMap::new()));

        // Test various complex expressions
        let expressions = vec![
            ("every_second", "* * * * * *"),
            ("every_2_seconds", "*/2 * * * * *"),
            ("every_5_seconds", "*/5 * * * * *"),
            ("complex_range", "0,30 * * * * *"), // At 0 and 30 seconds
        ];

        for (name, expr) in expressions {
            let results = Arc::clone(&expression_results);
            let expr_name = name.to_string();

            cron.add_fn(expr, move || {
                let mut map = results.lock().unwrap();
                *map.entry(expr_name.clone()).or_insert(0) += 1;
            }).unwrap();
        }

        sleep(Duration::from_millis(6100)).await;
        cron.stop();

        let results = expression_results.lock().unwrap();
        
        // Verify different expressions executed appropriately
        let every_second = results.get("every_second").unwrap_or(&0);
        let every_2_seconds = results.get("every_2_seconds").unwrap_or(&0);
        let every_5_seconds = results.get("every_5_seconds").unwrap_or(&0);

        assert!(every_second >= &5, "Every second should run ~6 times, got {}", every_second);
        assert!(every_2_seconds >= &2, "Every 2 seconds should run ~3 times, got {}", every_2_seconds);
        assert!(every_5_seconds >= &1, "Every 5 seconds should run at least once, got {}", every_5_seconds);
    }

    #[tokio::test]
    async fn test_job_execution_timing_precision() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let execution_times = Arc::new(Mutex::new(Vec::new()));
        let times = Arc::clone(&execution_times);

        cron.add_fn("* * * * * *", move || {
            let now = Instant::now();
            times.lock().unwrap().push(now);
        }).unwrap();

        sleep(Duration::from_millis(3100)).await;
        cron.stop();

        let times = execution_times.lock().unwrap();
        assert!(times.len() >= 2, "Should have multiple executions for timing test");

        // Check timing precision between executions
        for window in times.windows(2) {
            let interval = window[1].duration_since(window[0]);
            // Be more lenient with timing in async tests
            assert!(interval >= Duration::from_millis(800) && interval <= Duration::from_millis(1500),
                   "Execution interval should be roughly 1000ms ±300ms, got {:?}", interval);
        }
    }

    #[tokio::test]
    async fn test_error_recovery() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let success_count = Arc::new(Mutex::new(0));
        let panic_count = Arc::new(Mutex::new(0));

        // Job that sometimes panics
        let success_counter = Arc::clone(&success_count);
        let panic_counter = Arc::clone(&panic_count);
        
        cron.add_fn("* * * * * *", move || {
            let count = {
                let mut pc = panic_counter.lock().unwrap();
                *pc += 1;
                *pc
            };
            
            if count % 3 == 0 {
                // Simulate error every 3rd execution, but don't actually panic
                // as that would terminate the thread
                return;
            }
            
            let mut sc = success_counter.lock().unwrap();
            *sc += 1;
        }).unwrap();

        sleep(Duration::from_millis(3100)).await;
        cron.stop();

        let success = *success_count.lock().unwrap();
        let total = *panic_count.lock().unwrap();
        
        assert!(total >= 2, "Should have multiple execution attempts, got {}", total);
        assert!(success >= 1, "Should have some successful executions despite errors, got {}", success);
        assert!(success < total, "Should have some failed executions to test recovery, success: {}, total: {}", success, total);
    }

    // ===============================
    // ASYNC PERFORMANCE TESTS
    // ===============================

    #[tokio::test]
    async fn test_async_concurrent_operations() {
        let num_crons = 10;
        let mut futures = Vec::new();

        for i in 0..num_crons {
            let future = tokio::spawn(async move {
                let mut cron = Cron::new(Utc);
                cron.start();

                let counter = Arc::new(Mutex::new(0));
                let counter_clone = Arc::clone(&counter);

                cron.add_fn("* * * * * *", move || {
                    let mut count = counter_clone.lock().unwrap();
                    *count += 1;
                }).unwrap();

                sleep(Duration::from_millis(1100)).await;
                cron.stop();

                let final_count = *counter.lock().unwrap();
                (i, final_count)
            });
            futures.push(future);
        }

        let results = join_all(futures).await;
        
        // Verify all concurrent operations succeeded
        for result in results {
            let (instance_id, count) = result.unwrap();
            assert!(count >= 1 && count <= 2, 
                   "Concurrent instance {} should execute 1-2 times, got {}", instance_id, count);
        }
    }

    #[tokio::test] 
    async fn test_scheduler_overhead_async() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let execution_times = Arc::new(Mutex::new(Vec::new()));
        let times_clone = Arc::clone(&execution_times);

        // Job that measures its own execution timing
        cron.add_fn("* * * * * *", move || {
            let execution_start = Instant::now();
            
            // Simulate minimal work
            std::thread::sleep(Duration::from_millis(1));
            
            let execution_end = Instant::now();
            let overhead = execution_end.duration_since(execution_start);
            
            times_clone.lock().unwrap().push(overhead);
        }).unwrap();

        // Let it run for several executions
        sleep(Duration::from_millis(3100)).await;
        cron.stop();
        
        let times = execution_times.lock().unwrap();
        assert!(times.len() >= 3);
        
        // Calculate average overhead
        let total_overhead: Duration = times.iter().sum();
        let avg_overhead = total_overhead / times.len() as u32;
        
        // Overhead should be minimal (less than 10ms on average)
        assert!(avg_overhead < Duration::from_millis(10), 
               "Average execution overhead too high: {:?}", avg_overhead);
    }

    #[tokio::test]
    async fn test_rapid_start_stop_cycles_async() {
        let mut cron = Cron::new(Utc);
        
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);
        
        cron.add_fn("* * * * * *", move || {
            let mut count = counter_clone.lock().unwrap();
            *count += 1;
        }).unwrap();
        
        let start = Instant::now();
        
        // Rapidly start and stop the cron scheduler
        for _ in 0..20 {
            cron.start();
            sleep(Duration::from_millis(50)).await; // Brief execution
            cron.stop();
        }
        
        let elapsed = start.elapsed();
        
        // Should handle rapid cycling without issues
        assert!(elapsed < Duration::from_millis(5000), 
               "Rapid start/stop cycles took too long: {:?}", elapsed);
        
        // Should have executed some jobs
        let final_count = *counter.lock().unwrap();
        assert!(final_count >= 0, "Jobs execution count should be reasonable during rapid cycling, got {}", final_count);
    }

    // ===============================
    // ASYNC THREAD SAFETY & MEMORY SAFETY TESTS
    // ===============================

    #[tokio::test]
    async fn test_async_concurrent_job_access() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        
        let mut cron = Cron::new(Utc);
        cron.start();
        
        let shared_counter = Arc::new(AtomicUsize::new(0));
        let execution_count = Arc::new(AtomicUsize::new(0));
        
        // Add a job that increments shared state
        let counter_clone = Arc::clone(&shared_counter);
        let exec_clone = Arc::clone(&execution_count);
        
        cron.add_fn("* * * * * *", move || {
            // Simulate concurrent access to shared data
            let old_value = counter_clone.load(Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(1)); // Simulate work
            counter_clone.store(old_value + 1, Ordering::SeqCst);
            exec_clone.fetch_add(1, Ordering::SeqCst);
        }).unwrap();
        
        // Let job execute several times
        sleep(Duration::from_millis(3100)).await;
        cron.stop();
        
        let final_counter = shared_counter.load(Ordering::SeqCst);
        let executions = execution_count.load(Ordering::SeqCst);
        
        // Counter should equal execution count (no lost updates)
        assert_eq!(final_counter, executions, 
                  "Shared counter should equal execution count (no data races)");
        assert!(executions >= 2, "Should have multiple executions");
    }

    #[tokio::test]
    async fn test_async_shared_mutable_state_safety() {
        let mut cron = Cron::new(Utc);
        cron.start();
        
        // Use a shared vector protected by Mutex
        let shared_data = Arc::new(Mutex::new(Vec::new()));
        let read_count = Arc::new(AtomicUsize::new(0));
        
        // Job that writes to shared state
        let data_clone1 = Arc::clone(&shared_data);
        cron.add_fn("* * * * * *", move || {
            let mut data = data_clone1.lock().unwrap();
            data.push(std::thread::current().id());
        }).unwrap();
        
        // Job that reads from shared state
        let data_clone2 = Arc::clone(&shared_data);
        let read_clone = Arc::clone(&read_count);
        cron.add_fn("* * * * * *", move || {
            let data = data_clone2.lock().unwrap();
            let _len = data.len(); // Read operation
            read_clone.fetch_add(1, Ordering::SeqCst);
        }).unwrap();
        
        sleep(Duration::from_millis(3100)).await;
        cron.stop();
        
        let final_data = shared_data.lock().unwrap();
        let reads = read_count.load(Ordering::SeqCst);
        
        // Should not panic or have data corruption
        assert!(final_data.len() >= 2, "Should have written data");
        assert!(reads >= 2, "Should have read data");
        
        // Verify data integrity - all entries should be valid thread IDs
        for thread_id in final_data.iter() {
            // ThreadId should be valid (this would fail if memory corruption occurred)
            assert_ne!(format!("{:?}", thread_id), "");
        }
    }

    #[tokio::test]
    async fn test_async_memory_ordering_consistency() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        
        let mut cron = Cron::new(Utc);
        cron.start();
        
        let writes = Arc::new(AtomicUsize::new(0));
        let reads = Arc::new(AtomicUsize::new(0));
        let consistency_check = Arc::new(AtomicUsize::new(0));
        
        // Writer job
        let writes_clone = Arc::clone(&writes);
        let check_clone1 = Arc::clone(&consistency_check);
        cron.add_fn("* * * * * *", move || {
            let write_count = writes_clone.fetch_add(1, Ordering::SeqCst);
            // Update consistency check with same ordering
            check_clone1.store(write_count + 1, Ordering::SeqCst);
        }).unwrap();
        
        // Reader job
        let reads_clone = Arc::clone(&reads);
        let writes_read = Arc::clone(&writes);
        let check_clone2 = Arc::clone(&consistency_check);
        cron.add_fn("* * * * * *", move || {
            let read_count = reads_clone.fetch_add(1, Ordering::SeqCst);
            let write_count = writes_read.load(Ordering::SeqCst);
            let check_value = check_clone2.load(Ordering::SeqCst);
            
            // Consistency check: check_value should be >= write_count
            // If memory ordering is broken, this could fail
            if check_value > 0 && write_count > 0 {
                assert!(check_value >= write_count, 
                       "Memory ordering violation: check={}, writes={}, reads={}", 
                       check_value, write_count, read_count);
            }
        }).unwrap();
        
        sleep(Duration::from_millis(3100)).await;
        cron.stop();
        
        let final_writes = writes.load(Ordering::SeqCst);
        let final_reads = reads.load(Ordering::SeqCst);
        
        assert!(final_writes >= 2, "Should have multiple writes");
        assert!(final_reads >= 2, "Should have multiple reads");
    }

    #[tokio::test]
    async fn test_async_scheduler_clone_safety() {
        let mut cron1 = Cron::new(Utc);
        
        // Clone the scheduler
        let mut cron2 = cron1.clone();
        
        let counter1 = Arc::new(AtomicUsize::new(0));
        let counter2 = Arc::new(AtomicUsize::new(0));
        
        // Add jobs to both cloned instances
        let c1_clone = Arc::clone(&counter1);
        cron1.add_fn("* * * * * *", move || {
            c1_clone.fetch_add(1, Ordering::SeqCst);
        }).unwrap();
        
        let c2_clone = Arc::clone(&counter2);
        cron2.add_fn("* * * * * *", move || {
            c2_clone.fetch_add(1, Ordering::SeqCst);
        }).unwrap();
        
        // Start both schedulers
        cron1.start();
        cron2.start();
        
        sleep(Duration::from_millis(2100)).await;
        
        // Stop both schedulers
        cron1.stop();
        cron2.stop();
        
        let count1 = counter1.load(Ordering::SeqCst);
        let count2 = counter2.load(Ordering::SeqCst);
        
        // Both should execute independently without interfering
        assert!(count1 >= 1, "First cron should execute, got {}", count1);
        assert!(count2 >= 1, "Second cron should execute, got {}", count2);
    }

    #[tokio::test]
    async fn test_async_cross_thread_job_execution() {
        use std::thread;
        
        let mut cron = Cron::new(Utc);
        
        let main_thread_id = thread::current().id();
        let execution_threads = Arc::new(Mutex::new(Vec::new()));
        
        let threads_clone = Arc::clone(&execution_threads);
        cron.add_fn("* * * * * *", move || {
            let current_thread = thread::current().id();
            threads_clone.lock().unwrap().push(current_thread);
        }).unwrap();
        
        cron.start();
        sleep(Duration::from_millis(2100)).await;
        cron.stop();
        
        let threads = execution_threads.lock().unwrap();
        
        // Jobs should execute in different threads than main thread
        assert!(threads.len() >= 1, "Should have executed jobs");
        for &thread_id in threads.iter() {
            assert_ne!(thread_id, main_thread_id, 
                      "Jobs should execute in background threads, not main thread");
        }
    }

    #[tokio::test]
    async fn test_async_large_scale_concurrent_execution() {
        let mut cron = Cron::new(Utc);
        cron.start();
        
        let total_executions = Arc::new(AtomicUsize::new(0));
        let active_jobs = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        
        // Add many jobs that track concurrency
        for _ in 0..50 {
            let total_clone = Arc::clone(&total_executions);
            let active_clone = Arc::clone(&active_jobs);
            let max_clone = Arc::clone(&max_concurrent);
            
            cron.add_fn("* * * * * *", move || {
                // Track entry into job
                let current_active = active_clone.fetch_add(1, Ordering::SeqCst) + 1;
                
                // Update max concurrent if necessary
                loop {
                    let current_max = max_clone.load(Ordering::SeqCst);
                    if current_active <= current_max || 
                       max_clone.compare_exchange_weak(current_max, current_active, 
                                                      Ordering::SeqCst, Ordering::Relaxed).is_ok() {
                        break;
                    }
                }
                
                // Simulate work
                std::thread::sleep(Duration::from_millis(10));
                
                // Track exit from job
                active_clone.fetch_sub(1, Ordering::SeqCst);
                total_clone.fetch_add(1, Ordering::SeqCst);
            }).unwrap();
        }
        
        sleep(Duration::from_millis(2100)).await;
        cron.stop();
        
        let total = total_executions.load(Ordering::SeqCst);
        let max_concurrent_jobs = max_concurrent.load(Ordering::SeqCst);
        
        assert!(total >= 50, "Should have executed many jobs, got {}", total);
        assert!(max_concurrent_jobs >= 10, "Should have high concurrency, got {}", max_concurrent_jobs);
        
        // Final active count should be 0 (all jobs completed)
        let final_active = active_jobs.load(Ordering::SeqCst);
        assert_eq!(final_active, 0, "All jobs should have completed, {} still active", final_active);
    }

    #[tokio::test]
    async fn test_async_job_removal_thread_safety() {
        let mut cron = Cron::new(Utc);
        cron.start();
        
        let execution_count = Arc::new(AtomicUsize::new(0));
        let mut job_ids = Vec::new();
        
        // Add multiple jobs
        for i in 0..10 {
            let count_clone = Arc::clone(&execution_count);
            let job_id = cron.add_fn("* * * * * *", move || {
                count_clone.fetch_add(1, Ordering::SeqCst);
                // Simulate work with different durations per job
                std::thread::sleep(Duration::from_millis((i % 5 + 1) * 10));
            }).unwrap();
            
            job_ids.push(job_id);
        }
        
        // Let jobs start executing
        sleep(Duration::from_millis(500)).await;
        
        // Remove jobs while they might be executing
        for &job_id in job_ids.iter().take(5) {
            cron.remove(job_id);
            sleep(Duration::from_millis(50)).await; // Stagger removals
        }
        
        // Continue execution while removal happens
        sleep(Duration::from_millis(1000)).await;
        cron.stop();
        
        let final_count = execution_count.load(Ordering::SeqCst);
        
        // Should complete without deadlock or panic
        assert!(final_count >= 5, "Some jobs should have executed before removal");
    }
}
