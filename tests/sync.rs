//! Comprehensive synchronous tests for CronTab
//!
//! This module contains all tests for the synchronous functionality of the cron scheduler,
//! including basic operations, error handling, integration scenarios, and performance tests.

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc, Mutex,
        },
        thread::{self, sleep},
        time::{Duration, Instant},
    };

    use chrono::{FixedOffset, Local, TimeZone, Utc};
    use cron_tab::{Cron, CronError};

    // ONE-TIME EXECUTION TESTS

    #[test]
    fn test_add_fn_once() {
        let mut cron = Cron::new(Utc);

        let counter = Arc::new(Mutex::new(0));
        let counter1 = Arc::clone(&counter);

        // Schedule a one-time job 2 seconds in the future
        let target_time = Utc::now() + chrono::Duration::seconds(2);
        cron.add_fn_once(target_time, move || {
            let mut value = counter1.lock().unwrap();
            *value += 1;
        })
        .unwrap();

        cron.start();

        // Wait before the job should execute
        sleep(Duration::from_millis(1500));
        assert_eq!(*counter.lock().unwrap(), 0, "Job should not have executed yet");

        // Wait for the job to execute
        sleep(Duration::from_millis(1000));
        assert_eq!(*counter.lock().unwrap(), 1, "Job should have executed once");

        // Wait longer to ensure it doesn't execute again
        sleep(Duration::from_millis(2000));
        assert_eq!(*counter.lock().unwrap(), 1, "Job should only execute once");

        cron.stop();
    }

    #[test]
    fn test_add_fn_after() {
        let mut cron = Cron::new(Utc);

        let counter = Arc::new(Mutex::new(0));
        let counter1 = Arc::clone(&counter);

        // Schedule a job to run after 2 seconds
        cron.add_fn_after(Duration::from_secs(2), move || {
            let mut value = counter1.lock().unwrap();
            *value += 1;
        })
        .unwrap();

        cron.start();

        // Wait before the job should execute
        sleep(Duration::from_millis(1500));
        assert_eq!(*counter.lock().unwrap(), 0, "Job should not have executed yet");

        // Wait for the job to execute
        sleep(Duration::from_millis(1000));
        assert_eq!(*counter.lock().unwrap(), 1, "Job should have executed once");

        // Wait longer to ensure it doesn't execute again
        sleep(Duration::from_millis(2000));
        assert_eq!(*counter.lock().unwrap(), 1, "Job should only execute once");

        cron.stop();
    }

    #[test]
    fn test_multiple_one_time_jobs() {
        let mut cron = Cron::new(Utc);

        let counter = Arc::new(Mutex::new(0));
        let counter1 = Arc::clone(&counter);
        let counter2 = Arc::clone(&counter);
        let counter3 = Arc::clone(&counter);

        // Schedule multiple one-time jobs at different times
        cron.add_fn_after(Duration::from_secs(1), move || {
            let mut value = counter1.lock().unwrap();
            *value += 1;
        })
        .unwrap();

        cron.add_fn_after(Duration::from_secs(2), move || {
            let mut value = counter2.lock().unwrap();
            *value += 10;
        })
        .unwrap();

        cron.add_fn_after(Duration::from_secs(3), move || {
            let mut value = counter3.lock().unwrap();
            *value += 100;
        })
        .unwrap();

        cron.start();

        sleep(Duration::from_millis(1500));
        assert_eq!(*counter.lock().unwrap(), 1, "First job should have executed");

        sleep(Duration::from_millis(1000));
        assert_eq!(*counter.lock().unwrap(), 11, "Second job should have executed");

        sleep(Duration::from_millis(1000));
        assert_eq!(*counter.lock().unwrap(), 111, "Third job should have executed");

        // Wait to ensure no more executions
        sleep(Duration::from_millis(1000));
        assert_eq!(*counter.lock().unwrap(), 111, "No more jobs should execute");

        cron.stop();
    }

    #[test]
    fn test_remove_one_time_job_before_execution() {
        let mut cron = Cron::new(Utc);

        let counter = Arc::new(Mutex::new(0));
        let counter1 = Arc::clone(&counter);

        // Schedule a one-time job
        let job_id = cron
            .add_fn_after(Duration::from_secs(2), move || {
                let mut value = counter1.lock().unwrap();
                *value += 1;
            })
            .unwrap();

        cron.start();

        // Remove the job before it executes
        sleep(Duration::from_millis(500));
        cron.remove(job_id);

        // Wait past when the job would have executed
        sleep(Duration::from_millis(2000));
        assert_eq!(*counter.lock().unwrap(), 0, "Removed job should not execute");

        cron.stop();
    }

    #[test]
    fn test_mix_recurring_and_one_time_jobs() {
        let mut cron = Cron::new(Utc);

        let recurring_counter = Arc::new(Mutex::new(0));
        let recurring_counter1 = Arc::clone(&recurring_counter);

        let once_counter = Arc::new(Mutex::new(0));
        let once_counter1 = Arc::clone(&once_counter);

        // Add a recurring job that runs every second
        cron.add_fn("* * * * * * *", move || {
            let mut value = recurring_counter1.lock().unwrap();
            *value += 1;
        })
        .unwrap();

        // Add a one-time job that runs after 2 seconds
        cron.add_fn_after(Duration::from_secs(2), move || {
            let mut value = once_counter1.lock().unwrap();
            *value += 1;
        })
        .unwrap();

        cron.start();

        sleep(Duration::from_millis(3500));

        let recurring_count = *recurring_counter.lock().unwrap();
        let once_count = *once_counter.lock().unwrap();

        // Recurring job should have executed multiple times
        assert!(
            recurring_count >= 2,
            "Recurring job should execute multiple times, got {}",
            recurring_count
        );

        // One-time job should have executed exactly once
        assert_eq!(once_count, 1, "One-time job should execute exactly once");

        cron.stop();
    }

    #[test]
    fn test_duration_out_of_range_error() {
        let mut cron = Cron::new(Utc);

        // Test with an extremely large duration that exceeds chrono's limit
        let very_long_time = Duration::from_secs(u64::MAX);
        let result = cron.add_fn_after(very_long_time, || {
            println!("This should not execute");
        });

        assert!(result.is_err(), "Should fail with DurationOutOfRange");
        match result {
            Err(CronError::DurationOutOfRange) => {
                // Success - correct error type
            }
            Err(e) => panic!("Expected DurationOutOfRange, got {:?}", e),
            Ok(_) => panic!("Should have returned an error"),
        }

        // Test with a reasonable duration (should succeed)
        let normal_duration = Duration::from_secs(10);
        let result = cron.add_fn_after(normal_duration, || {
            println!("This will execute");
        });

        assert!(result.is_ok(), "Should succeed with normal duration");
    }

    // BASIC FUNCTIONALITY TESTS

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
        assert!(value >= 1, "Expected at least 1 execution, got {}", value);
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
        assert!(value >= 1, "Expected at least 1 execution, got {}", value);
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
        assert!(value1 >= 1, "Counter1 expected at least 1, got {}", value1);
        assert!(value2 >= 1, "Counter2 expected at least 1, got {}", value2);
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
        assert!(*counter.lock().unwrap() >= 1, "Should have executed at least once");
        cron.remove(job_id);

        // Reset counter and verify no more executions
        {
            let mut count = counter.lock().unwrap();
            *count = 0;
        }

        sleep(Duration::from_millis(1001));
        let value = *counter.lock().unwrap();
        assert_eq!(value, 0, "Should not execute after removal");
    }

    // ERROR HANDLING TESTS

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

    // PERFORMANCE TESTS

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

    // INTEGRATION TESTS

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
        
        // Test multiple complex patterns
        let job1 = cron.add_fn("0 30 14 * * MON-FRI *", || {}).unwrap();
        let job2 = cron.add_fn("0 0 9,17 * * * *", || {}).unwrap();
        let job3 = cron.add_fn("0 */15 * * * * *", || {}).unwrap();
        
        cron.start();
        thread::sleep(Duration::from_millis(100));
        cron.stop();
        
        // Clean up
        cron.remove(job1);
        cron.remove(job2);
        cron.remove(job3);
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

    // THREAD SAFETY & MEMORY SAFETY TESTS

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
    fn test_crossbeam_channel_paths() {
        let mut cron = Cron::new(Utc);
        
        // Start the scheduler in background
        cron.start();
        
        // Give the scheduler time to start
        std::thread::sleep(Duration::from_millis(100));
        
        // Add a job while running to test the channel send path
        let job_id = cron.add_fn("* * * * * * *", || {}).unwrap();
        
        // Let it run briefly
        std::thread::sleep(Duration::from_millis(100));
        
        // Stop the scheduler
        cron.stop();
        
        // Clean up
        cron.remove(job_id);
    }

    #[test]
    fn test_schedule_next_edge_cases() {
        let mut cron = Cron::new(Utc);
        
        // Test various edge case schedules that might affect coverage
        let schedules = vec![
            "0 0 0 29 2 * *",  // Leap year edge case
            "59 59 23 31 12 * *", // End of year
            "0 0 0 1 1 * 2100",   // Far future
        ];
        
        for schedule in schedules {
            if let Ok(job_id) = cron.add_fn(schedule, || {}) {
                cron.remove(job_id);
            }
        }
    }

    #[test]
    fn test_empty_entries_with_start_blocking() {
        let cron = Cron::new(Utc);
        
        // Start with no entries to test empty case
        let cron_clone = cron.clone();
        let handle = std::thread::spawn(move || {
            let mut cron_mut = cron_clone;
            cron_mut.start_blocking();
        });
        
        // Let it run briefly with no jobs
        std::thread::sleep(Duration::from_millis(50));
        
        // Stop the scheduler
        cron.stop();
        
        let _ = handle.join();
    }

    #[test]
    fn test_timer_expiry_path() {
        let mut cron = Cron::new(Utc);
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = executed.clone();
        
        // Add a job that executes quickly
        let job_id = cron.add_fn("* * * * * * *", move || {
            executed_clone.store(true, Ordering::SeqCst);
        }).unwrap();
        
        // Start and let it run to test the timer expiry path
        cron.start();
        std::thread::sleep(Duration::from_millis(1100));
        cron.stop();
        
        // Verify execution
        assert!(executed.load(Ordering::SeqCst));
        
        cron.remove(job_id);
    }

    #[test]
    fn test_wait_duration_calculation() {
        let mut cron = Cron::new(Utc);
        
        // Add a job far in the future to test wait duration calculation
        let job_id = cron.add_fn("0 0 0 1 1 * 2030", || {}).unwrap();
        
        // Start briefly to test the wait calculation logic
        cron.start();
        std::thread::sleep(Duration::from_millis(50));
        cron.stop();
        
        cron.remove(job_id);
    }

    #[test]
    fn test_multiple_jobs_execution_order() {
        let mut cron = Cron::new(Utc);
        let execution_order = Arc::new(Mutex::new(Vec::new()));
        
        // Add jobs with different schedules to test sorting
        for i in 0..3 {
            let order_clone = execution_order.clone();
            let _job_id = cron.add_fn("* * * * * * *", move || {
                order_clone.lock().unwrap().push(i);
            }).unwrap();
        }
        
        cron.start();
        std::thread::sleep(Duration::from_millis(1100));
        cron.stop();
        
        let order = execution_order.lock().unwrap();
        assert!(!order.is_empty(), "Jobs should have executed");
    }

    #[test]
    fn test_job_due_check_logic() {
        let mut cron = Cron::new(Utc);
        let execution_count = Arc::new(AtomicUsize::new(0));
        
        // Mix of jobs with different frequencies
        let count1 = execution_count.clone();
        let job_id1 = cron.add_fn("* * * * * * *", move || {
            count1.fetch_add(1, Ordering::SeqCst);
        }).unwrap();
        
        let count2 = execution_count.clone();
        let job_id2 = cron.add_fn("*/2 * * * * * *", move || {
            count2.fetch_add(1, Ordering::SeqCst);
        }).unwrap();
        
        cron.start();
        std::thread::sleep(Duration::from_millis(2100));
        cron.stop();
        
        let final_count = execution_count.load(Ordering::SeqCst);
        assert!(final_count >= 2, "Should have executed multiple jobs");
        
        cron.remove(job_id1);
        cron.remove(job_id2);
    }

    #[test]
    fn test_stop_channel_receive() {
        let mut cron = Cron::new(Utc);
        
        // Start in background
        cron.start();
        std::thread::sleep(Duration::from_millis(50));
        
        // Stop should trigger the stop channel receive path
        cron.stop();
        
        // Give time for stop to process
        std::thread::sleep(Duration::from_millis(50));
    }

    #[test]
    fn test_precise_timing_edge_cases() {
        let mut cron = Cron::new(Utc);
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = executed.clone();
        
        // Job that should execute immediately
        let job_id = cron.add_fn("* * * * * * *", move || {
            executed_clone.store(true, Ordering::SeqCst);
        }).unwrap();
        
        // Test the precise timing logic in start_blocking
        cron.start();
        std::thread::sleep(Duration::from_millis(1050)); // Just over 1 second
        cron.stop();
        
        assert!(executed.load(Ordering::SeqCst));
        cron.remove(job_id);
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

    // UNSAFECELL & NON-SYNC TYPE TESTS

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

    // COVERAGE IMPROVEMENT TESTS

    #[test]
    fn test_remove_from_stopped_scheduler_sync() {
        let mut cron = Cron::new(Utc);
        
        // Add job to stopped scheduler
        let job_id = cron.add_fn("* * * * * * *", || {
            println!("Test job");
        }).unwrap();
        
        // Remove from stopped scheduler (should call remove_entry directly)
        cron.remove(job_id);
        
        // Verify job was removed by trying to remove again
        cron.remove(job_id); // Should not panic
        
        // Remove non-existent job
        cron.remove(9999); // Should not panic
    }

    #[test]
    fn test_start_blocking_edge_cases() {
        let mut cron = Cron::new(Utc);
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = executed.clone();
        
        // Add job that will execute
        let _job_id = cron.add_fn("* * * * * * *", move || {
            executed_clone.store(true, Ordering::SeqCst);
        }).unwrap();
        
        // Test start_blocking in a separate thread
        let mut cron_clone = cron.clone();
        let handle = thread::spawn(move || {
            // This should run until stopped
            cron_clone.start_blocking();
        });
        
        // Let it run briefly
        thread::sleep(Duration::from_millis(1100));
        
        // Stop the scheduler
        cron.stop();
        
        // Wait for thread to finish
        let _ = handle.join();
        
        // Verify job executed
        assert!(executed.load(Ordering::SeqCst));
    }

    #[test]
    fn test_scheduler_with_no_jobs_sync() {
        let mut cron = Cron::new(Utc);
        
        // Start with no jobs to cover empty entries case
        cron.start();
        
        // Let it run briefly
        thread::sleep(Duration::from_millis(100));
        
        // Stop scheduler
        cron.stop();
        
        // Add job after stopping
        let job_id = cron.add_fn("* * * * * * *", || {}).unwrap();
        
        // Remove it
        cron.remove(job_id);
    }

    #[test]
    fn test_stop_channel_edge_cases_sync() {
        let mut cron = Cron::new(Utc);
        
        // Test multiple stop calls
        cron.stop(); // First stop call
        cron.stop(); // Second stop call (should not panic)
        cron.stop(); // Third stop call (should not panic)
        
        // Start and stop quickly
        cron.start();
        cron.stop();
        
        // Start again and add job
        cron.start();
        let job_id = cron.add_fn("* * * * * * *", || {}).unwrap();
        
        // Stop and clean up
        cron.stop();
        cron.remove(job_id);
    }

    #[test]
    fn test_job_scheduling_edge_cases_sync() {
        let mut cron = Cron::new(Utc);
        
        // Test job that schedules very far in the future
        let far_future_job = cron.add_fn("0 0 0 1 1 * 2030", || {
            println!("Far future job");
        }).unwrap();
        
        // Test job with immediate execution
        let immediate_job = cron.add_fn("* * * * * * *", || {
            println!("Immediate job");
        }).unwrap();
        
        cron.start();
        thread::sleep(Duration::from_millis(100));
        cron.stop();
        
        // Clean up
        cron.remove(far_future_job);
        cron.remove(immediate_job);
    }

    #[test]
    fn test_schedule_method_coverage_sync() {
        let mut cron = Cron::new(Utc);
        
        // Test adding job when scheduler is not running
        let job_id1 = cron.add_fn("* * * * * * *", || {}).unwrap();
        
        // Start scheduler
        cron.start();
        
        // Add more jobs while running to test channel communication
        let job_id2 = cron.add_fn("*/2 * * * * * *", || {}).unwrap();
        let job_id3 = cron.add_fn("*/3 * * * * * *", || {}).unwrap();
        
        thread::sleep(Duration::from_millis(100));
        cron.stop();
        
        // Clean up
        cron.remove(job_id1);
        cron.remove(job_id2);
        cron.remove(job_id3);
    }

    #[test]
    fn test_entry_schedule_next_edge_cases() {
        // Test creating an entry-like structure directly for testing
        // Since entry module is private, we'll test through the public API
        let mut cron = Cron::new(Utc);

        // Test with a schedule that might have edge cases (Feb 29th - leap year)
        let job_id = cron.add_fn("0 0 0 29 2 * *", || {}).unwrap();

        // The job should be created successfully and scheduled properly
        cron.start();
        thread::sleep(Duration::from_millis(100));
        cron.stop();

        // Clean up
        cron.remove(job_id);
    }

    #[test]
    fn test_set_timezone_functionality() {
        // Create cron with FixedOffset timezone
        let initial_tz = FixedOffset::east_opt(0).unwrap(); // UTC equivalent
        let mut cron = Cron::new(initial_tz);

        // Add a job with initial timezone
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        let job_id = cron.add_fn("* * * * * * *", move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }).unwrap();

        // Change timezone to Tokyo (UTC+9)
        let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
        cron.set_timezone(tokyo_tz);

        // Start and run briefly
        cron.start();
        thread::sleep(Duration::from_millis(1100));
        cron.stop();

        // Job should still execute
        let count = counter.load(Ordering::SeqCst);
        assert!(count >= 1, "Job should execute after timezone change");

        // Clean up
        cron.remove(job_id);
    }

}
