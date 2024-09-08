#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        thread::sleep,
        time::Duration,
    };

    use chrono::{FixedOffset, Local, TimeZone};
    use cron_tab::Cron;

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
        assert_eq!(value, 2)
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
        assert_eq!(value, 2)
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
        assert_eq!(value1, 2);
        assert_eq!(value2, 1);
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
        assert_eq!(*counter.lock().unwrap(), 1);
        cron.remove(job_id);

        sleep(Duration::from_millis(1001));
        let value = *counter.lock().unwrap();
        assert_eq!(value, 1)
    }
}
