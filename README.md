
# CronTab
[![Build](https://github.com/tuyentv96/rust-crontab/actions/workflows/build.yml/badge.svg?branch=master)](https://github.com/tuyentv96/rust-crontab/actions/workflows/build.yml) [![](https://docs.rs/cron_tab/badge.svg)](https://docs.rs/cron_tab) [![](https://img.shields.io/crates/v/cron_tab.svg)](https://crates.io/crates/cron_tab) [![codecov](https://codecov.io/gh/tuyentv96/rust-crontab/branch/master/graph/badge.svg?token=YF89MDH26R)](https://codecov.io/gh/tuyentv96/rust-crontab)

A cron job library for Rust.

# Features
- **Sync and Async Support:** Manage cron jobs in both synchronous and asynchronous contexts.
- **Flexible Cron Expressions:** Use standard cron expressions for job scheduling.

## Usage

Please see the [Documentation](https://docs.rs/cron_tab/) for more details.

Add to your `Cargo.toml`:

```toml
[dependencies]
cron_tab = {version = "0.2", features = ["sync", "async"]}
```

The cron expression format:

```text
sec   min   hour   day of month   month   day of week   year
*     *     *      *              *       *             *
```

Example:

```rust
extern crate cron_tab;  
  
use chrono::{FixedOffset, Local, TimeZone, Utc};  
  
fn main() {  
 let local_tz = Local::from_offset(&FixedOffset::east(7));  
 let utc_tz = Utc;  
 
 // create new cron with timezone
 let mut cron = cron_tab::Cron::new(utc_tz);  
  
 // add test fn to cron
 let job_test_id = cron.add_fn("* * * * * * *", test).unwrap();  
  
 // start cron
 cron.start();  

 // sleep 2 second
 std::thread::sleep(std::time::Duration::from_secs(2));
 
 // add one more function
 let anonymous_job_id = cron.add_fn("* * * * * *", || {  
            println!("anonymous fn");  
 }).unwrap();  
  
 // remove job_test  
 cron.remove(job_test_id);  
  
 // sleep 2 second
 std::thread::sleep(std::time::Duration::from_secs(2));
  
 // stop cron  
 cron.stop();  
}  
  
fn test() {  
    println!("now: {}", Local::now().to_string());  
}
```

Async Example:
```rust
use std::sync::Arc;

use chrono::{FixedOffset, Local, TimeZone};
use cron_tab::AsyncCron;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let local_tz = Local::from_offset(&FixedOffset::east(7));
    let mut cron = AsyncCron::new(local_tz);

    let first_job_id = cron.add_fn("* * * * * *", print_now).await;

    cron.start().await;

    let counter = Arc::new(Mutex::new(1));
    cron.add_fn("* * * * * *", move || {
        let counter = counter.clone();
        async move {
            let mut counter = counter.lock().await;
            *counter += 1;
            let now = Local::now().to_string();
            println!("{} counter value: {}", now, counter);
        }
    })
    .await;

    std::thread::sleep(std::time::Duration::from_secs(10));

    // stop cron
    cron.stop();
}

async fn print_now() {
    println!("now: {}", Local::now().to_string());
}
```

## License
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)
