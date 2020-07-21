extern crate cron_tab;

use chrono::{FixedOffset, Local, TimeZone, Utc};

fn main() {
    let local_tz = Local::from_offset(&FixedOffset::east(7));
    let utc_tz = Utc;

    let mut cron = cron_tab::Cron::new(utc_tz);

    let job_test_id = cron.add_fn("* * * * * * *", test).unwrap();

    cron.start();

    std::thread::sleep(std::time::Duration::from_secs(2));
    let anonymous_job_id = cron
        .add_fn("* * * * * *", || {
            println!("anonymous fn");
        })
        .unwrap();

    // remove job_test
    cron.remove(job_test_id);

    loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

fn test() {
    println!("now: {}", Local::now().to_string());
}
