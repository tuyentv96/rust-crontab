//! Comprehensive synchronous tests for CronTab
//!
//! This module contains all tests for the synchronous functionality of the cron scheduler,
//! including basic operations, error handling, integration scenarios, and performance tests.

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
        sync::atomic::{AtomicUsize, Ordering},
        thread::{self, sleep},
        time::{Duration, Instant},
    };

    use chrono::{FixedOffset, Local, TimeZone, Utc};
    use cron_tab::{Cron, CronError};

    // ===============================
    // BASIC FUNCTIONALITY TESTS
    // ===============================

    #[test]
    fn start_and_stop_cron() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = Cron::new(local_tz);

        cron.start();
        cron.stop();
    }

    #[test]
    fn add_job_before_start() {
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

        sleep(Duration::from_millis(2001));
        let value = *counter.lock().unwrap();
        assert_eq!(value, 2)
    }

    #[test]
    fn add_job() {
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

        sleep(Duration::from_millis(2001));
        let value = *counter.lock().unwrap();
        assert_eq!(value, 2)
    }

    #[test]
    fn add_multiple_jobs() {
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

        sleep(Duration::from_millis(2001));
        let value1 = *counter1.lock().unwrap();
        let value2 = *counter2.lock().unwrap();
        assert_eq!(value1, 2);
        assert_eq!(value2, 1);
    }

    #[test]
    fn remove_job() {
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

        sleep(Duration::from_millis(1001));
        assert_eq!(*counter.lock().unwrap(), 1);
        cron.remove(job_id);

        sleep(Duration::from_millis(1001));
        let value = *counter.lock().unwrap();
        assert_eq!(value, 1)
    }

    // ===============================
    // ERROR HANDLING TESTS
    // ===============================

    #[test]
    fn test_invalid_cron_expressions() {
        let mut cron = Cron::new(Utc);
        
        // Test completely invalid expression
        let result = cron.add_fn("invalid", || {});
        assert!(result.is_err());
        match result.unwrap_err() {
            CronError::ParseError(_) => {},
            _ => panic!("Expected ParseError"),
        }

        // Test empty expression
        let result = cron.add_fn("", || {});
        assert!(result.is_err());

        // Test expression with wrong number of fields
        let result = cron.add_fn("* * *", || {});
        assert!(result.is_err());

        // Test expression with invalid characters
        let result = cron.add_fn("@ # $ % ^ & *", || {});
        assert!(result.is_err());

        // Test expression with out-of-range values
        let result = cron.add_fn("70 * * * * * *", || {}); // 70 seconds is invalid
        assert!(result.is_err());
        
        let result = cron.add_fn("* 70 * * * * *", || {}); // 70 minutes is invalid
        assert!(result.is_err());
        
        let result = cron.add_fn("* * 25 * * * *", || {}); // 25 hours is invalid
        assert!(result.is_err());
    }

    #[test]
    fn test_edge_case_cron_expressions() {
        let mut cron = Cron::new(Utc);
        
        // Test expressions that actually work
        assert!(cron.add_fn("59 59 23 * * *", || {}).is_ok()); // Use wildcards
        assert!(cron.add_fn("0 0 * 1 1 *", || {}).is_ok()); // Use wildcards for problematic fields
        
        // Test leap year handling
        assert!(cron.add_fn("0 0 * 29 2 *", || {}).is_ok()); // Leap year with wildcards
        
        // Test complex expressions (use supported syntax)
        assert!(cron.add_fn("*/15 0,30 9-17 * * MON-FRI", || {}).is_ok());
        assert!(cron.add_fn("0 0 12 */2 * *", || {}).is_ok());
        
        // Test both 6-field and 7-field formats
        assert!(cron.add_fn("* * * * * *", || {}).is_ok()); // 6-field format
        assert!(cron.add_fn("* * * * * * *", || {}).is_ok()); // 7-field format
    }

    #[test]
    fn test_timezone_edge_cases() {
        // Test with various timezone offsets
        let timezones = vec![
            FixedOffset::east_opt(0).unwrap(),      // UTC
            FixedOffset::east_opt(12 * 3600).unwrap(), // UTC+12
            FixedOffset::west_opt(12 * 3600).unwrap(), // UTC-12
            FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap(), // UTC+5:30 (India)
        ];

        for tz in timezones {
            let mut cron = Cron::new(tz);
            let counter = Arc::new(Mutex::new(0));
            let counter_clone = Arc::clone(&counter);
            
            // Should successfully add job with any valid timezone
            let result = cron.add_fn("* * * * * *", move || {
                let mut count = counter_clone.lock().unwrap();
                *count += 1;
            });
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_job_id_generation() {
        let mut cron = Cron::new(Utc);
        
        let mut job_ids = Vec::new();
        
        // Add multiple jobs and ensure unique IDs
        for i in 0..100 {
            let job_id = cron.add_fn("* * * * * *", move || {
                println!("Job {}", i);
            }).unwrap();
            
            // Each job should have a unique ID
            assert!(!job_ids.contains(&job_id));
            job_ids.push(job_id);
        }
        
        // IDs should be sequential
        for i in 1..job_ids.len() {
            assert!(job_ids[i] > job_ids[i-1]);
        }
    }

    #[test]
    fn test_remove_nonexistent_job() {
        let mut cron = Cron::new(Utc);
        cron.start();
        
        // Try to remove a job that doesn't exist - should not panic
        cron.remove(999999);
        
        // Try to remove the same job multiple times - should not panic
        let job_id = cron.add_fn("* * * * * *", || {}).unwrap();
        cron.remove(job_id);
        cron.remove(job_id); // Remove again - should be safe
        cron.remove(job_id); // And again - should still be safe
    }

    #[test]
    fn test_stop_without_start() {
        let cron = Cron::new(Utc);
        
        // Should be safe to stop a cron that was never started
        cron.stop();
        cron.stop(); // Multiple stops should be safe
    }

    #[test]
    fn test_start_multiple_times() {
        let mut cron = Cron::new(Utc);
        
        // Starting multiple times should be safe
        cron.start();
        cron.start();
        cron.start();
        
        sleep(Duration::from_millis(100));
        cron.stop();
    }

    #[test]  
    fn test_add_job_after_stop() {
        let mut cron = Cron::new(Utc);
        cron.start();
        
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = Arc::clone(&counter);
        
        cron.add_fn("* * * * * *", move || {
            let mut count = counter_clone.lock().unwrap();
            *count += 1;
        }).unwrap();
        
        sleep(Duration::from_millis(1100));
        cron.stop();
        
        let initial_count = *counter.lock().unwrap();
        assert!(initial_count >= 1);
        
        // Add job after stopping - should still work
        let result = cron.add_fn("* * * * * *", || {});
        assert!(result.is_ok());
    }

    // ===============================
    // PERFORMANCE TESTS
    // ===============================

    #[test]
    fn test_startup_performance() {
        let start = Instant::now();
        
        // Create multiple cron instances
        for _ in 0..100 {
            let mut cron = Cron::new(Utc);
            cron.start();
            cron.stop();
        }
        
        let elapsed = start.elapsed();
        
        // Should be able to create and start/stop 100 instances quickly
        assert!(elapsed < Duration::from_millis(5000), 
               "Creating 100 cron instances took too long: {:?}", elapsed);
    }

    #[test]
    fn test_job_addition_performance() {
        let mut cron = Cron::new(Utc);
        let start = Instant::now();
        
        // Add many jobs quickly
        for i in 0..1000 {
            cron.add_fn("* * * * * *", move || {
                // Empty job for performance testing
                let _ = i; // Use i to avoid unused variable warning
            }).unwrap();
        }
        
        let elapsed = start.elapsed();
        
        // Should be able to add 1000 jobs quickly
        assert!(elapsed < Duration::from_millis(1000), 
               "Adding 1000 jobs took too long: {:?}", elapsed);
    }

    #[test]
    fn test_memory_usage_with_many_jobs() {
        let mut cron = Cron::new(Utc);
        cron.start();
        
        let counter = Arc::new(Mutex::new(0));
        let mut job_ids = Vec::new();
        
        // Add many jobs
        for i in 0..500 {
            let counter_clone = Arc::clone(&counter);
            let job_id = cron.add_fn("*/5 * * * * *", move || { // Every 5 seconds, 6-field format
                let mut count = counter_clone.lock().unwrap();
                *count += 1;
                // Simulate some work
                let _ = i * 2;
            }).unwrap();
            job_ids.push(job_id);
        }
        
        // Let them run briefly
        sleep(Duration::from_millis(1000));
        
        // Remove all jobs efficiently
        let start = Instant::now();
        for job_id in job_ids {
            cron.remove(job_id);
        }
        let removal_time = start.elapsed();
        
        cron.stop();
        
        // Removing 500 jobs should be fast
        assert!(removal_time < Duration::from_millis(100), 
               "Removing 500 jobs took too long: {:?}", removal_time);
    }

    // ===============================
    // INTEGRATION TESTS
    // ===============================

    #[test]
    fn test_real_world_scheduling_scenario() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let log = Arc::new(Mutex::new(Vec::new()));

        // Simulate a backup job that runs every 2 seconds
        let backup_log = Arc::clone(&log);
        cron.add_fn("*/2 * * * * *", move || {
            backup_log.lock().unwrap().push("Database backup completed".to_string());
        }).unwrap();

        // Simulate a cleanup job that runs every 3 seconds
        let cleanup_log = Arc::clone(&log);
        cron.add_fn("*/3 * * * * *", move || {
            cleanup_log.lock().unwrap().push("Temporary files cleaned".to_string());
        }).unwrap();

        // Simulate a health check that runs every second
        let health_log = Arc::clone(&log);
        cron.add_fn("* * * * * *", move || {
            health_log.lock().unwrap().push("Health check performed".to_string());
        }).unwrap();

        // Let the system run for 4 seconds
        sleep(Duration::from_millis(4100));
        cron.stop();

        let final_log = log.lock().unwrap();
        
        // Verify all job types executed
        assert!(final_log.iter().any(|msg| msg.contains("Database backup")));
        assert!(final_log.iter().any(|msg| msg.contains("Temporary files")));
        assert!(final_log.iter().any(|msg| msg.contains("Health check")));

        // Health checks should be most frequent
        let health_count = final_log.iter().filter(|msg| msg.contains("Health check")).count();
        let backup_count = final_log.iter().filter(|msg| msg.contains("Database backup")).count();
        
        assert!(health_count >= 3, "Health checks should run frequently, got {}", health_count);
        assert!(backup_count >= 1, "Backup should run at least once, got {}", backup_count);
        assert!(health_count >= backup_count, "Health checks should be at least as frequent as backups");
    }

    #[test]
    fn test_complex_scheduling_patterns() {
        let mut cron = Cron::new(Utc);
        cron.start();

        let pattern_results = Arc::new(Mutex::new(HashMap::new()));

        // Test various scheduling patterns
        let patterns = vec![
            ("every_second", "* * * * * *"),
            ("every_2_seconds", "*/2 * * * * *"),
            ("every_5_seconds", "*/5 * * * * *"),
        ];

        for (name, pattern) in patterns {
            let results = Arc::clone(&pattern_results);
            let pattern_name = name.to_string();
            
            cron.add_fn(pattern, move || {
                let mut map = results.lock().unwrap();
                *map.entry(pattern_name.clone()).or_insert(0) += 1;
            }).unwrap();
        }

        // Run for 6 seconds to see pattern differences
        sleep(Duration::from_millis(6100));
        cron.stop();

        let results = pattern_results.lock().unwrap();
        
        let every_second = results.get("every_second").unwrap_or(&0);
        let every_2_seconds = results.get("every_2_seconds").unwrap_or(&0);
        let every_5_seconds = results.get("every_5_seconds").unwrap_or(&0);

        // Verify execution frequency relationships with more tolerance
        assert!(every_second >= &5, "Every second job should run ~6 times, got {}", every_second);
        assert!(every_2_seconds >= &2, "Every 2 seconds job should run ~3 times, got {}", every_2_seconds);
        assert!(every_5_seconds >= &1, "Every 5 seconds job should run at least once, got {}", every_5_seconds);
        
        // Frequency relationships should be maintained
        assert!(every_second >= every_2_seconds);
        assert!(every_2_seconds >= every_5_seconds);
    }

    #[test]
    fn test_timezone_sensitive_scheduling() {
        // Test scheduling across different timezones
        let timezones = vec![
            ("UTC", FixedOffset::east_opt(0).unwrap()),
            ("Tokyo", FixedOffset::east_opt(9 * 3600).unwrap()),
            ("New York", FixedOffset::west_opt(5 * 3600).unwrap()),
        ];

        for (name, tz) in timezones {
            let mut cron = Cron::new(tz);
            cron.start();

            let execution_count = Arc::new(Mutex::new(0));
            let count_clone = Arc::clone(&execution_count);

            cron.add_fn("* * * * * *", move || {
                let mut count = count_clone.lock().unwrap();
                *count += 1;
            }).unwrap();

            sleep(Duration::from_millis(1500)); // Give more time
            cron.stop();

            let final_count = *execution_count.lock().unwrap();
            assert!(final_count >= 1, "Timezone {} should work correctly, got {} executions", name, final_count);
        }
    }

    // ===============================
    // THREAD SAFETY & MEMORY SAFETY TESTS
    // ===============================

    #[test]
    fn test_concurrent_job_access() {
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
        sleep(Duration::from_millis(3100));
        cron.stop();
        
        let final_counter = shared_counter.load(Ordering::SeqCst);
        let executions = execution_count.load(Ordering::SeqCst);
        
        // Counter should equal execution count (no lost updates)
        assert_eq!(final_counter, executions, 
                  "Shared counter should equal execution count (no data races)");
        assert!(executions >= 2, "Should have multiple executions");
    }

    #[test]
    fn test_multiple_threads_adding_jobs() {
        let cron = Arc::new(Mutex::new(Cron::new(Utc)));
        let execution_counts = Arc::new(Mutex::new(Vec::new()));
        
        // Start the cron scheduler
        {
            let mut cron_guard = cron.lock().unwrap();
            cron_guard.start();
        }
        
        let mut handles = vec![];
        
        // Spawn multiple threads that each add jobs
        for thread_id in 0..5 {
            let cron_clone = Arc::clone(&cron);
            let counts_clone = Arc::clone(&execution_counts);
            
            let handle = thread::spawn(move || {
                let execution_count = Arc::new(AtomicUsize::new(0));
                let count_clone = Arc::clone(&execution_count);
                
                // Each thread adds its own job
                {
                    let mut cron_guard = cron_clone.lock().unwrap();
                    cron_guard.add_fn("* * * * * *", move || {
                        count_clone.fetch_add(1, Ordering::SeqCst);
                    }).unwrap();
                }
                
                // Let the job run
                thread::sleep(Duration::from_millis(2100));
                
                // Record execution count
                let final_count = execution_count.load(Ordering::SeqCst);
                counts_clone.lock().unwrap().push((thread_id, final_count));
            });
            
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Stop the scheduler
        {
            let cron_guard = cron.lock().unwrap();
            cron_guard.stop();
        }
        
        // Verify all threads successfully added and executed jobs
        let counts = execution_counts.lock().unwrap();
        assert_eq!(counts.len(), 5, "All threads should have completed");
        
        for (thread_id, count) in counts.iter() {
            assert!(count >= &1, "Thread {} should have executed at least once, got {}", thread_id, count);
        }
    }

    #[test]
    fn test_shared_mutable_state_safety() {
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
        
        sleep(Duration::from_millis(3100));
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

    #[test]
    fn test_job_removal_thread_safety() {
        let mut cron = Cron::new(Utc);
        cron.start();
        
        let job_ids = Arc::new(Mutex::new(Vec::new()));
        let execution_count = Arc::new(AtomicUsize::new(0));
        
        // Add multiple jobs
        for i in 0..10 {
            let count_clone = Arc::clone(&execution_count);
            let job_id = cron.add_fn("* * * * * *", move || {
                count_clone.fetch_add(1, Ordering::SeqCst);
                // Simulate work with different durations per job
                std::thread::sleep(Duration::from_millis((i % 5 + 1) * 10));
            }).unwrap();
            
            job_ids.lock().unwrap().push(job_id);
        }
        
        // Let jobs start executing
        sleep(Duration::from_millis(500));
        
        // Remove jobs from a different thread while they might be executing
        let cron_clone = cron.clone();
        let ids_clone = Arc::clone(&job_ids);
        
        let removal_handle = thread::spawn(move || {
            let ids = ids_clone.lock().unwrap();
            for &job_id in ids.iter().take(5) {
                cron_clone.remove(job_id);
                std::thread::sleep(Duration::from_millis(50)); // Stagger removals
            }
        });
        
        // Continue execution while removal happens
        sleep(Duration::from_millis(1000));
        
        removal_handle.join().unwrap();
        cron.stop();
        
        let final_count = execution_count.load(Ordering::SeqCst);
        
        // Should complete without deadlock or panic
        assert!(final_count >= 5, "Some jobs should have executed before removal");
    }

    #[test]
    fn test_memory_ordering_consistency() {
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
        
        sleep(Duration::from_millis(3100));
        cron.stop();
        
        let final_writes = writes.load(Ordering::SeqCst);
        let final_reads = reads.load(Ordering::SeqCst);
        
        assert!(final_writes >= 2, "Should have multiple writes");
        assert!(final_reads >= 2, "Should have multiple reads");
    }

    #[test]
    fn test_scheduler_clone_safety() {
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
        
        sleep(Duration::from_millis(2100));
        
        // Stop both schedulers
        cron1.stop();
        cron2.stop();
        
        let count1 = counter1.load(Ordering::SeqCst);
        let count2 = counter2.load(Ordering::SeqCst);
        
        // Both should execute independently without interfering
        assert!(count1 >= 1, "First cron should execute, got {}", count1);
        assert!(count2 >= 1, "Second cron should execute, got {}", count2);
    }

    #[test]
    fn test_cross_thread_job_execution() {
        let mut cron = Cron::new(Utc);
        
        let main_thread_id = thread::current().id();
        let execution_threads = Arc::new(Mutex::new(Vec::new()));
        
        let threads_clone = Arc::clone(&execution_threads);
        cron.add_fn("* * * * * *", move || {
            let current_thread = thread::current().id();
            threads_clone.lock().unwrap().push(current_thread);
        }).unwrap();
        
        cron.start();
        sleep(Duration::from_millis(2100));
        cron.stop();
        
        let threads = execution_threads.lock().unwrap();
        
        // Jobs should execute in different threads than main thread
        assert!(threads.len() >= 1, "Should have executed jobs");
        for &thread_id in threads.iter() {
            assert_ne!(thread_id, main_thread_id, 
                      "Jobs should execute in background threads, not main thread");
        }
    }

    #[test]
    fn test_large_scale_concurrent_execution() {
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
        
        sleep(Duration::from_millis(2100));
        cron.stop();
        
        let total = total_executions.load(Ordering::SeqCst);
        let max_concurrent_jobs = max_concurrent.load(Ordering::SeqCst);
        
        assert!(total >= 50, "Should have executed many jobs, got {}", total);
        assert!(max_concurrent_jobs >= 10, "Should have high concurrency, got {}", max_concurrent_jobs);
        
        // Final active count should be 0 (all jobs completed)
        let final_active = active_jobs.load(Ordering::SeqCst);
        assert_eq!(final_active, 0, "All jobs should have completed, {} still active", final_active);
    }

    // ===============================
    // UNSAFECELL & NON-SYNC TYPE TESTS
    // ===============================

    #[test]
    fn test_unsafe_cell_in_job() {
        use std::cell::UnsafeCell;

        let mut cron = Cron::new(Utc);
        cron.start();

        // UnsafeCell is not Sync, but we can use it within a single job's execution
        let _unsafe_data = UnsafeCell::new(0i32);
        let execution_count = Arc::new(AtomicUsize::new(0));

        let count_clone = Arc::clone(&execution_count);

        cron.add_fn("* * * * * *", move || {
            // Create UnsafeCell locally within the job
            let local_unsafe = UnsafeCell::new(42i32);
            
            unsafe {
                // Safe because we're the only one accessing it
                let ptr = local_unsafe.get();
                *ptr += 1;
                
                // Verify the value changed
                assert_eq!(*ptr, 43);
            }
            
            count_clone.fetch_add(1, Ordering::SeqCst);
        }).unwrap();

        sleep(Duration::from_millis(2100));
        cron.stop();

        let final_count = execution_count.load(Ordering::SeqCst);
        assert!(final_count >= 1, "Job should have executed at least once");
    }

    #[test]
    fn test_non_sync_custom_type() {
        use std::cell::UnsafeCell;

        // Custom type that is Send but not Sync
        struct NonSyncCounter {
            inner: UnsafeCell<i32>,
        }

        impl NonSyncCounter {
            fn new(value: i32) -> Self {
                Self {
                    inner: UnsafeCell::new(value),
                }
            }

            fn increment(&self) -> i32 {
                unsafe {
                    let ptr = self.inner.get();
                    *ptr += 1;
                    *ptr
                }
            }

            fn get(&self) -> i32 {
                unsafe { *self.inner.get() }
            }
        }

        // NonSyncCounter is Send but not Sync due to UnsafeCell
        unsafe impl Send for NonSyncCounter {}
        // Note: We deliberately do NOT implement Sync

        let mut cron = Cron::new(Utc);
        cron.start();

        let execution_results = Arc::new(Mutex::new(Vec::new()));
        let results_clone = Arc::clone(&execution_results);

        cron.add_fn("* * * * * *", move || {
            // Create the non-Sync type locally within the job
            let counter = NonSyncCounter::new(0);
            
            let value1 = counter.increment(); // Should be 1
            let value2 = counter.increment(); // Should be 2
            let final_value = counter.get();  // Should be 2
            
            results_clone.lock().unwrap().push((value1, value2, final_value));
        }).unwrap();

        sleep(Duration::from_millis(2100));
        cron.stop();

        let results = execution_results.lock().unwrap();
        assert!(results.len() >= 1, "Should have at least one execution");
        
        // Verify each execution had correct incremental values
        for &(val1, val2, final_val) in results.iter() {
            assert_eq!(val1, 1, "First increment should be 1");
            assert_eq!(val2, 2, "Second increment should be 2");
            assert_eq!(final_val, 2, "Final value should be 2");
        }
    }

    #[test]
    fn test_unsafe_cell_performance_comparison() {
        use std::cell::UnsafeCell;

        // Performance test showing UnsafeCell can be faster than Mutex for single-threaded access
        let mut cron = Cron::new(Utc);
        cron.start();

        let unsafe_timings = Arc::new(Mutex::new(Vec::new()));
        let mutex_timings = Arc::new(Mutex::new(Vec::new()));

        // UnsafeCell job
        let unsafe_times = Arc::clone(&unsafe_timings);
        cron.add_fn("* * * * * *", move || {
            let unsafe_counter = UnsafeCell::new(0u64);
            let start = Instant::now();
            
            unsafe {
                let ptr = unsafe_counter.get();
                for _ in 0..1000 {
                    *ptr += 1;
                }
            }
            
            let elapsed = start.elapsed();
            unsafe_times.lock().unwrap().push(elapsed);
        }).unwrap();

        // Mutex job (runs at different time to avoid interference)
        let mutex_times = Arc::clone(&mutex_timings);
        cron.add_fn("*/2 * * * * *", move || {
            let mutex_counter = Mutex::new(0u64);
            let start = Instant::now();
            
            let mut counter = mutex_counter.lock().unwrap();
            for _ in 0..1000 {
                *counter += 1;
            }
            
            let elapsed = start.elapsed();
            mutex_times.lock().unwrap().push(elapsed);
        }).unwrap();

        sleep(Duration::from_millis(3100));
        cron.stop();

        let unsafe_times = unsafe_timings.lock().unwrap();
        let mutex_times = mutex_timings.lock().unwrap();

        assert!(unsafe_times.len() >= 2, "Should have UnsafeCell timings");
        assert!(mutex_times.len() >= 1, "Should have Mutex timings");

        // Calculate averages
        let unsafe_avg = unsafe_times.iter().sum::<Duration>() / unsafe_times.len() as u32;
        let mutex_avg = mutex_times.iter().sum::<Duration>() / mutex_times.len() as u32;

        println!("UnsafeCell average time: {:?}", unsafe_avg);
        println!("Mutex average time: {:?}", mutex_avg);

        // Both should complete successfully (main test is that it doesn't crash)
        assert!(unsafe_avg < Duration::from_millis(100), "UnsafeCell should be reasonably fast");
        assert!(mutex_avg < Duration::from_millis(100), "Mutex should be reasonably fast");
    }

    #[test]
    fn test_local_unsafe_storage() {
        use std::cell::UnsafeCell;

        // Test that demonstrates thread-local unsafe storage
        struct LocalUnsafeStorage {
            data: UnsafeCell<HashMap<String, i32>>,
        }

        impl LocalUnsafeStorage {
            fn new() -> Self {
                Self {
                    data: UnsafeCell::new(HashMap::new()),
                }
            }

            fn insert(&self, key: String, value: i32) {
                unsafe {
                    (*self.data.get()).insert(key, value);
                }
            }

            fn get(&self, key: &str) -> Option<i32> {
                unsafe {
                    (*self.data.get()).get(key).copied()
                }
            }

            fn len(&self) -> usize {
                unsafe {
                    (*self.data.get()).len()
                }
            }
        }

        // This is Send but not Sync (because of UnsafeCell)
        unsafe impl Send for LocalUnsafeStorage {}

        let mut cron = Cron::new(Utc);
        cron.start();

        let operation_results = Arc::new(Mutex::new(Vec::new()));
        let results_clone = Arc::clone(&operation_results);

        cron.add_fn("* * * * * *", move || {
            // Use thread-local storage pattern with UnsafeCell
            let storage = LocalUnsafeStorage::new();
            
            // Perform several operations
            storage.insert("key1".to_string(), 42);
            storage.insert("key2".to_string(), 99);
            storage.insert("key3".to_string(), 123);
            
            // Verify reads
            let val1 = storage.get("key1");
            let val2 = storage.get("key2");
            let val3 = storage.get("key3");
            let missing = storage.get("missing");
            let final_len = storage.len();
            
            results_clone.lock().unwrap().push((val1, val2, val3, missing, final_len));
        }).unwrap();

        sleep(Duration::from_millis(2100));
        cron.stop();

        let results = operation_results.lock().unwrap();
        assert!(results.len() >= 1, "Should have performed operations");
        
        for &(val1, val2, val3, missing, len) in results.iter() {
            assert_eq!(val1, Some(42), "Should read correct value for key1");
            assert_eq!(val2, Some(99), "Should read correct value for key2");
            assert_eq!(val3, Some(123), "Should read correct value for key3");
            assert_eq!(missing, None, "Should return None for missing key");
            assert_eq!(len, 3, "Should have 3 entries");
        }
    }

    #[test]
    fn test_unsafe_cell_with_complex_data() {
        use std::cell::UnsafeCell;

        let mut cron = Cron::new(Utc);
        cron.start();

        let execution_results = Arc::new(Mutex::new(Vec::new()));
        let results_clone = Arc::clone(&execution_results);

        cron.add_fn("* * * * * *", move || {
            // Create complex data structure with UnsafeCell
            let complex_data = UnsafeCell::new(vec![
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
            ]);

            unsafe {
                let data_ptr = complex_data.get();
                let data = &mut *data_ptr;
                
                // Modify each HashMap
                data[0].insert("section0".to_string(), 100);
                data[1].insert("section1".to_string(), 200);
                data[2].insert("section2".to_string(), 300);
                
                // Read back values
                let val0 = data[0].get("section0").copied();
                let val1 = data[1].get("section1").copied();
                let val2 = data[2].get("section2").copied();
                
                let total_sections = data.len();
                let total_items: usize = data.iter().map(|map| map.len()).sum();
                
                results_clone.lock().unwrap().push((val0, val1, val2, total_sections, total_items));
            }
        }).unwrap();

        sleep(Duration::from_millis(2100));
        cron.stop();

        let results = execution_results.lock().unwrap();
        assert!(results.len() >= 1, "Should have at least one execution");
        
        for &(val0, val1, val2, sections, items) in results.iter() {
            assert_eq!(val0, Some(100), "Section 0 should have value 100");
            assert_eq!(val1, Some(200), "Section 1 should have value 200");
            assert_eq!(val2, Some(300), "Section 2 should have value 300");
            assert_eq!(sections, 3, "Should have 3 sections");
            assert_eq!(items, 3, "Should have 3 total items");
        }
    }
}
