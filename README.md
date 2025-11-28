# CronTab

[![Build](https://github.com/tuyentv96/rust-crontab/actions/workflows/build.yml/badge.svg?branch=master)](https://github.com/tuyentv96/rust-crontab/actions/workflows/build.yml) 
[![](https://docs.rs/cron_tab/badge.svg)](https://docs.rs/cron_tab) 
[![](https://img.shields.io/crates/v/cron_tab.svg)](https://crates.io/crates/cron_tab) 
[![codecov](https://codecov.io/gh/tuyentv96/rust-crontab/branch/master/graph/badge.svg?token=YF89MDH26R)](https://codecov.io/gh/tuyentv96/rust-crontab)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A lightweight, thread-safe cron job scheduler for Rust with support for both sync and async execution.

## Features

- ⚡ **Sync & Async** - Choose the execution mode that fits your application
- 🌍 **Timezone Aware** - Full timezone support via chrono
- 🎯 **Second Precision** - 7-field cron expressions with second-level scheduling
- 🔧 **Runtime Control** - Add, remove, start, and stop jobs dynamically
- 🛡️ **Thread Safe** - Built with Rust's safety guarantees

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
cron_tab = { version = "0.2", features = ["sync", "async"] }
```

## Usage

### Synchronous Jobs

```rust
use chrono::Utc;
use cron_tab::Cron;

fn main() {
    let mut cron = Cron::new(Utc);

    // Add a job that runs every second
    cron.add_fn("* * * * * * *", || {
        println!("Running every second!");
    }).unwrap();

    cron.start();
    std::thread::sleep(std::time::Duration::from_secs(10));
    cron.stop();
}
```

### Asynchronous Jobs

```rust
use chrono::Utc;
use cron_tab::AsyncCron;

#[tokio::main]
async fn main() {
    let mut cron = AsyncCron::new(Utc);

    cron.add_fn("* * * * * *", || async {
        println!("Running every minute!");
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }).await.unwrap();

    cron.start().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    cron.stop().await;
}
```

## Cron Expression Format

CronTab uses 7-field cron expressions with second precision:

```
sec  min  hour  day  month  weekday  year
*    *    *     *    *      *        *
```

**Field ranges:**
- `sec`: 0-59
- `min`: 0-59  
- `hour`: 0-23
- `day`: 1-31
- `month`: 1-12
- `weekday`: 0-6 (Sunday=0)
- `year`: 1970-3000

**Common patterns:**

| Expression | Description |
|------------|-------------|
| `* * * * * * *` | Every second |
| `0 * * * * * *` | Every minute |
| `0 0 * * * * *` | Every hour |
| `0 0 0 * * * *` | Daily at midnight |
| `0 0 9 * * MON-FRI *` | Weekdays at 9 AM |
| `0 30 14 * * * *` | Daily at 2:30 PM |

## Advanced Examples

### Managing Jobs Dynamically

```rust
use chrono::Utc;
use cron_tab::Cron;

fn main() {
    let mut cron = Cron::new(Utc);
    cron.start();

    // Add a job and get its ID
    let job_id = cron.add_fn("*/5 * * * * * *", || {
        println!("Running every 5 seconds");
    }).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(15));

    // Remove the job dynamically
    cron.remove(job_id);
    
    cron.stop();
}
```

### Shared State in Async Jobs

```rust
use std::sync::Arc;
use chrono::Utc;
use cron_tab::AsyncCron;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let mut cron = AsyncCron::new(Utc);
    let counter = Arc::new(Mutex::new(0));
    
    let counter_clone = counter.clone();
    cron.add_fn("* * * * * * *", move || {
        let counter = counter_clone.clone();
        async move {
            let mut count = counter.lock().await;
            *count += 1;
            println!("Count: {}", *count);
        }
    }).await.unwrap();

    cron.start().await;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    cron.stop().await;
}
```

### Using Custom Timezones

```rust
use chrono::FixedOffset;
use cron_tab::Cron;

fn main() {
    // UTC+9 (Tokyo)
    let tokyo_tz = FixedOffset::east_opt(9 * 3600).unwrap();
    let mut cron = Cron::new(tokyo_tz);

    cron.add_fn("0 0 9 * * * *", || {
        println!("Good morning from Tokyo!");
    }).unwrap();

    cron.start();
    // Jobs run according to the specified timezone
}
```

## Feature Flags

Choose the features you need:

```toml
# Sync only (default)
cron_tab = "0.2"

# Async only
cron_tab = { version = "0.2", features = ["async"] }

# Both sync and async
cron_tab = { version = "0.2", features = ["all"] }
```

## Examples

Run the included examples:

```bash
# Synchronous example
cargo run --example simple --features sync

# Asynchronous example
cargo run --example async_simple --features async
```

## Testing

```bash
# All tests
cargo test --all-features

# Sync tests only
cargo test --features sync

# Async tests only
cargo test --features async
```

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Run `cargo fmt` and `cargo clippy --all-features`
5. Submit a pull request

For major changes, open an issue first to discuss your proposal.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Links

- **[Documentation](https://docs.rs/cron_tab)** - Full API reference
- **[Crates.io](https://crates.io/crates/cron_tab)** - Package registry
- **[Repository](https://github.com/tuyentv96/rust-crontab)** - Source code
