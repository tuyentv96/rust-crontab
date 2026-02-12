//! Example demonstrating one-time task execution at a specific time or after a delay.
//!
//! This example shows how to use the `add_fn_once` and `add_fn_after` methods
//! to schedule tasks that execute exactly once.

use chrono::{Duration, Utc};
use cron_tab::Cron;
use std::sync::{Arc, Mutex};

fn main() {
    println!("=== One-Time Task Execution Example ===\n");

    let mut cron = Cron::new(Utc);

    // Shared counter to track execution
    let counter = Arc::new(Mutex::new(0));

    // Example 1: Execute once after a delay
    println!("Scheduling job to execute after 2 seconds...");
    let counter1 = Arc::clone(&counter);
    cron.add_fn_after(std::time::Duration::from_secs(2), move || {
        let mut count = counter1.lock().unwrap();
        *count += 1;
        println!("[After 2s] Job executed! Counter: {}", *count);
    })
    .unwrap();

    // Example 2: Execute once at a specific datetime
    let target_time = Utc::now() + Duration::seconds(4);
    println!(
        "Scheduling job to execute at specific time: {}",
        target_time.format("%H:%M:%S")
    );
    let counter2 = Arc::clone(&counter);
    cron.add_fn_once(target_time, move || {
        let mut count = counter2.lock().unwrap();
        *count += 10;
        println!("[At {}] Job executed! Counter: {}", target_time.format("%H:%M:%S"), *count);
    })
    .unwrap();

    // Example 3: Execute once after 6 seconds
    println!("Scheduling job to execute after 6 seconds...");
    let counter3 = Arc::clone(&counter);
    cron.add_fn_after(std::time::Duration::from_secs(6), move || {
        let mut count = counter3.lock().unwrap();
        *count += 100;
        println!("[After 6s] Job executed! Counter: {}", *count);
    })
    .unwrap();

    // Example 4: Mix with recurring job
    println!("Scheduling recurring job every 3 seconds...");
    let recurring_counter = Arc::new(Mutex::new(0));
    let recurring_counter1 = Arc::clone(&recurring_counter);
    cron.add_fn("*/3 * * * * * *", move || {
        let mut count = recurring_counter1.lock().unwrap();
        *count += 1;
        println!("[Recurring] Executed {} times", *count);
    })
    .unwrap();

    // Start the scheduler
    println!("\nStarting scheduler...\n");
    cron.start();

    // Let it run for 8 seconds
    std::thread::sleep(std::time::Duration::from_secs(8));

    // Stop the scheduler
    cron.stop();

    println!("\n=== Execution Complete ===");
    println!("Final one-time counter: {}", *counter.lock().unwrap());
    println!("Final recurring counter: {}", *recurring_counter.lock().unwrap());
    println!("\nNote: One-time jobs execute exactly once and are automatically removed.");
}
