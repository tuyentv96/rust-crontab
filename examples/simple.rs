use chrono::{FixedOffset, Local, TimeZone};
use cron_tab;

fn main() {
    let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
    let mut cron = cron_tab::Cron::new(local_tz);

    let first_job_id = cron.add_fn("* * * * * * *", print_now).unwrap();

    // start cron in background
    cron.start();

    cron.add_fn("* * * * * *", move || {
        println!("add_fn {}", Local::now().to_string());
    })
    .unwrap();

    // remove job_test
    cron.remove(first_job_id);

    std::thread::sleep(std::time::Duration::from_secs(10));

    // stop cron
    cron.stop();
}

fn print_now() {
    println!("now: {}", Local::now().to_string());
}
