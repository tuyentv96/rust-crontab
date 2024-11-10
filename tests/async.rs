#[cfg(feature = "async")]
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{FixedOffset, Local, TimeZone};
    use cron_tab::AsyncCron;
    use tokio::sync::Mutex;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn start_and_stop_cron() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = AsyncCron::new(local_tz);

        cron.start().await;
        cron.stop().await;
    }

    #[tokio::test]
    async fn add_job() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = AsyncCron::new(local_tz);

        cron.start().await;

        let counter = Arc::new(Mutex::new(0));
        let counter1 = Arc::clone(&counter);

        cron.add_fn("* * * * * *", move || {
            let counter1 = Arc::clone(&counter1);
            async move {
                let mut value = counter1.lock().await;
                *value += 1;
            }
        })
        .await
        .unwrap();

        sleep(Duration::from_millis(2001)).await;
        let value = *counter.lock().await;
        assert_eq!(value, 2)
    }

    #[tokio::test]
    async fn add_multiple_jobs() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = AsyncCron::new(local_tz);

        cron.start().await;

        let counter1 = Arc::new(Mutex::new(0));
        let c1 = Arc::clone(&counter1);

        cron.add_fn("* * * * * *", move || {
            let counter = Arc::clone(&c1);
            async move {
                let mut value = counter.lock().await;
                *value += 1;
            }
        })
        .await
        .unwrap();

        let counter2 = Arc::new(Mutex::new(0));
        let c2 = Arc::clone(&counter2);
        cron.add_fn("*/2 * * * * *", move || {
            let counter = Arc::clone(&c2);
            async move {
                let mut value = counter.lock().await;
                *value += 1;
            }
        })
        .await
        .unwrap();

        sleep(Duration::from_millis(2001)).await;
        let value1 = *counter1.lock().await;
        let value2 = *counter2.lock().await;
        assert_eq!(value1, 2);
        assert_eq!(value2, 1);
    }

    #[tokio::test]
    async fn remove_job() {
        let local_tz = Local::from_offset(&FixedOffset::east_opt(7).unwrap());
        let mut cron = AsyncCron::new(local_tz);

        cron.start().await;

        let counter = Arc::new(Mutex::new(0));
        let counter1 = Arc::clone(&counter);

        let job_id = cron
            .add_fn("* * * * * *", move || {
                let counter1 = Arc::clone(&counter1);
                async move {
                    let mut value = counter1.lock().await;
                    *value += 1;
                }
            })
            .await
            .unwrap();

        sleep(Duration::from_millis(1001)).await;
        let value = *counter.lock().await;
        assert_eq!(value, 1);
        cron.remove(job_id).await;

        sleep(Duration::from_millis(1001)).await;
        let value = *counter.lock().await;
        assert_eq!(value, 1)
    }
}
