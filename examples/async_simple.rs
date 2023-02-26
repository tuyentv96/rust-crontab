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
