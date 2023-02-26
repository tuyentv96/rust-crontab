
# CronTab
[![](https://docs.rs/cron_tab/badge.svg)](https://docs.rs/cron_tab) [![](https://img.shields.io/crates/v/cron_tab.svg)](https://crates.io/crates/cron_tab) 

A cron job library for Rust.

## Usage

Please see the [Documentation](https://docs.rs/cron_tab/) for more details.

Add to your `Cargo.toml`:

```toml
[dependencies]
cron_tab = "0.2"
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

## License
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)
