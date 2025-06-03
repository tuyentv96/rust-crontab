# CronTab

[![Build](https://github.com/tuyentv96/rust-crontab/actions/workflows/build.yml/badge.svg?branch=master)](https://github.com/tuyentv96/rust-crontab/actions/workflows/build.yml) 
[![](https://docs.rs/cron_tab/badge.svg)](https://docs.rs/cron_tab) 
[![](https://img.shields.io/crates/v/cron_tab.svg)](https://crates.io/crates/cron_tab) 
[![codecov](https://codecov.io/gh/tuyentv96/rust-crontab/branch/master/graph/badge.svg?token=YF89MDH26R)](https://codecov.io/gh/tuyentv96/rust-crontab)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A modern, feature-rich cron job scheduler library for Rust applications with both synchronous and asynchronous execution support.

## ✨ Features

- 🔄 **Dual Execution Modes**: Support for both synchronous and asynchronous job execution
- 🌍 **Timezone Support**: Full timezone handling with chrono integration
- ⚡ **High Performance**: Efficient job scheduling with minimal overhead
- 🛡️ **Thread Safe**: Built with Rust's safety guarantees in mind
- 🎯 **Flexible Scheduling**: Standard cron expressions with second precision
- 🔧 **Runtime Control**: Add, remove, start, and stop jobs at runtime
- 📦 **Zero Config**: Works out of the box with sensible defaults

## 🚀 Quick Start

Add CronTab to your `Cargo.toml`:

```toml
[dependencies]
cron_tab = { version = "0.2", features = ["sync", "async"] }
```

### Basic Synchronous Usage

```rust
use chrono::{FixedOffset, Local, TimeZone};
use cron_tab::Cron;

fn main() {
    let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
    let mut cron = Cron::new(local_tz);

    // Add a job that runs every second
    cron.add_fn("* * * * * * *", || {
        println!("Job executed at: {}", Local::now());
    }).unwrap();

    // Start the scheduler
    cron.start();

    // Let it run for 10 seconds
    std::thread::sleep(std::time::Duration::from_secs(10));

    // Stop the scheduler
    cron.stop();
}
```

### Basic Asynchronous Usage

```rust
use chrono::{FixedOffset, Local, TimeZone};
use cron_tab::AsyncCron;

#[tokio::main]
async fn main() {
    let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
    let mut cron = AsyncCron::new(local_tz);

    // Add an async job
    cron.add_fn("* * * * * *", || async {
        println!("Async job executed at: {}", Local::now());
        // Simulate some async work
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }).await.unwrap();

    // Start the scheduler
    cron.start().await;

    // Let it run for 10 seconds
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // Stop the scheduler
    cron.stop().await;
}
```

## 📋 Cron Expression Format

CronTab supports 7-field cron expressions with second precision:

```
┌───────────── second (0 - 59)
│ ┌─────────── minute (0 - 59)
│ │ ┌───────── hour (0 - 23)
│ │ │ ┌─────── day of month (1 - 31)
│ │ │ │ ┌───── month (1 - 12)
│ │ │ │ │ ┌─── day of week (0 - 6) (Sunday to Saturday)
│ │ │ │ │ │ ┌─ year (1970 - 3000)
│ │ │ │ │ │ │
* * * * * * *
```

### Common Cron Patterns

| Expression | Description |
|------------|-------------|
| `* * * * * * *` | Every second |
| `0 * * * * * *` | Every minute |
| `0 0 * * * * *` | Every hour |
| `0 0 0 * * * *` | Every day at midnight |
| `0 0 9 * * MON-FRI *` | Every weekday at 9 AM |
| `0 0 0 1 * * *` | First day of every month |
| `0 30 14 * * * *` | Every day at 2:30 PM |

## 🔧 Advanced Usage

### Managing Jobs at Runtime

```rust
use chrono::Utc;
use cron_tab::Cron;

fn main() {
    let mut cron = Cron::new(Utc);
    cron.start();

    // Add a job and store its ID
    let job_id = cron.add_fn("*/5 * * * * * *", || {
        println!("This runs every 5 seconds");
    }).unwrap();

    // Let it run for a while
    std::thread::sleep(std::time::Duration::from_secs(15));

    // Remove the job
    cron.remove(job_id);
    println!("Job removed!");

    // Add a different job
    cron.add_fn("*/10 * * * * * *", || {
        println!("This runs every 10 seconds");
    }).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(30));
    cron.stop();
}
```

### Async Jobs with Shared State

```rust
use std::sync::Arc;
use chrono::Utc;
use cron_tab::AsyncCron;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let mut cron = AsyncCron::new(Utc);
    
    // Shared counter
    let counter = Arc::new(Mutex::new(0));
    
    // Job that increments counter
    let counter_clone = counter.clone();
    cron.add_fn("* * * * * * *", move || {
        let counter = counter_clone.clone();
        async move {
            let mut count = counter.lock().await;
            *count += 1;
            println!("Counter: {}", *count);
        }
    }).await.unwrap();

    cron.start().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    let final_count = *counter.lock().await;
    println!("Final count: {}", final_count);
    
    cron.stop().await;
}
```

### Different Timezones

```rust
use chrono::{FixedOffset, TimeZone, Utc};
use cron_tab::Cron;

fn main() {
    // Tokyo timezone (UTC+9)
    let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
    let mut cron_tokyo = Cron::new(tokyo_tz);

    // New York timezone (UTC-5)
    let ny_tz = FixedOffset::west_opt(5 * 3600).unwrap();
    let mut cron_ny = Cron::new(ny_tz);

    // UTC timezone
    let mut cron_utc = Cron::new(Utc);

    // Jobs will run according to their respective timezones
    cron_tokyo.add_fn("0 0 9 * * * *", || println!("Good morning from Tokyo!")).unwrap();
    cron_ny.add_fn("0 0 9 * * * *", || println!("Good morning from New York!")).unwrap();
    cron_utc.add_fn("0 0 9 * * * *", || println!("Good morning UTC!")).unwrap();

    cron_tokyo.start();
    cron_ny.start();
    cron_utc.start();

    // Let them run...
    std::thread::sleep(std::time::Duration::from_secs(30));
}
```

## ⚙️ Feature Flags

CronTab uses feature flags to minimize dependencies:

```toml
[dependencies]
# Include only sync support
cron_tab = { version = "0.2", features = ["sync"] }

# Include only async support  
cron_tab = { version = "0.2", features = ["async"] }

# Include both (default)
cron_tab = { version = "0.2", features = ["sync", "async"] }

# All features
cron_tab = { version = "0.2", features = ["all"] }
```

## 🧪 Running Examples

```bash
# Run the synchronous example
cargo run --example simple --features sync

# Run the asynchronous example  
cargo run --example async_simple --features async
```

## 🔍 API Documentation

For detailed API documentation, visit [docs.rs/cron_tab](https://docs.rs/cron_tab).

## 🛠️ Development

### Running Tests

```bash
# Run all tests
cargo test --all-features

# Run only sync tests
cargo test --features sync

# Run only async tests  
cargo test --features async
```

### Building Documentation

```bash
cargo doc --all-features --open
```

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Development Setup

1. Clone the repository
2. Install Rust (latest stable)
3. Run tests: `cargo test --all-features`
4. Check formatting: `cargo fmt`
5. Run clippy: `cargo clippy --all-features`

## 📄License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built with [cron](https://crates.io/crates/cron) for cron expression parsing
- Uses [chrono](https://crates.io/crates/chrono) for timezone handling
- Async support powered by [tokio](https://crates.io/crates/tokio)

---

<div align="center">

**[Documentation](https://docs.rs/cron_tab) • [Crates.io](https://crates.io/crates/cron_tab) • [Repository](https://github.com/tuyentv96/rust-crontab)**

</div>
