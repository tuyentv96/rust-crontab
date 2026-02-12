//! Example demonstrating async one-time task execution at a specific time or after a delay.
//!
//! This example shows how to use the `add_fn_once` and `add_fn_after` methods
//! with async functions to schedule tasks that execute exactly once.

use chrono::{Duration, Utc};
use cron_tab::AsyncCron;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    println!("=== Async One-Time Task Execution Example ===\n");

    let mut cron = AsyncCron::new(Utc);

    // Shared counter to track execution
    let counter = Arc::new(Mutex::new(0));

    // Example 1: Execute once after a delay
    println!("Scheduling async job to execute after 2 seconds...");
    let counter1 = Arc::clone(&counter);
    cron.add_fn_after(std::time::Duration::from_secs(2), move || {
        let counter = Arc::clone(&counter1);
        async move {
            let mut count = counter.lock().await;
            *count += 1;
            println!("[After 2s] Async job executed! Counter: {}", *count);

            // Simulate async work
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    // Example 2: Execute once at a specific datetime
    let target_time = Utc::now() + Duration::seconds(4);
    println!(
        "Scheduling async job to execute at specific time: {}",
        target_time.format("%H:%M:%S")
    );
    let counter2 = Arc::clone(&counter);
    cron.add_fn_once(target_time, move || {
        let counter = Arc::clone(&counter2);
        async move {
            let mut count = counter.lock().await;
            *count += 10;
            println!(
                "[At {}] Async job executed! Counter: {}",
                target_time.format("%H:%M:%S"),
                *count
            );

            // Simulate async work
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    // Example 3: Execute once after 6 seconds
    println!("Scheduling async job to execute after 6 seconds...");
    let counter3 = Arc::clone(&counter);
    cron.add_fn_after(std::time::Duration::from_secs(6), move || {
        let counter = Arc::clone(&counter3);
        async move {
            let mut count = counter.lock().await;
            *count += 100;
            println!("[After 6s] Async job executed! Counter: {}", *count);

            // Simulate async work
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    // Example 4: Mix with recurring job
    println!("Scheduling recurring async job every 3 seconds...");
    let recurring_counter = Arc::new(Mutex::new(0));
    let recurring_counter1 = Arc::clone(&recurring_counter);
    cron.add_fn("*/3 * * * * * *", move || {
        let counter = Arc::clone(&recurring_counter1);
        async move {
            let mut count = counter.lock().await;
            *count += 1;
            println!("[Recurring] Executed {} times", *count);

            // Simulate async work
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();

    // Start the scheduler
    println!("\nStarting scheduler...\n");
    cron.start().await;

    // Let it run for 8 seconds
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;

    // Stop the scheduler
    cron.stop().await;

    println!("\n=== Execution Complete ===");
    println!("Final one-time counter: {}", *counter.lock().await);
    println!(
        "Final recurring counter: {}",
        *recurring_counter.lock().await
    );
    println!("\nNote: One-time jobs execute exactly once and are automatically removed.");
}
