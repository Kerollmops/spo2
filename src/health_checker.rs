use std::time::Duration;

use futures::sink::SinkExt;
use futures::channel::mpsc::Sender;
use futures_timer::{Delay, TryFutureExt};

use redismodule::ThreadSafeContext;
use url::Url;
use crate::ReportStatus;

const TIMEOUT:      Duration = Duration::from_secs(5);
const NORMAL_PING:  Duration = Duration::from_secs(3);
const FAST_PING:    Duration = Duration::from_millis(800);

type ArrayDeque10<T> = arraydeque::ArrayDeque<[T; 10], arraydeque::Wrapping>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Healthy,
    Unhealthy,
    Unreacheable,
}

use Status::{Healthy, Unhealthy, Unreacheable};

impl Status {
    fn is_good(&self) -> bool {
        *self == Healthy
    }
}

pub async fn health_checker(url: Url, mut sender: Sender<(Url, ReportStatus)>) {
    let ctx = ThreadSafeContext::create();
    let mut last_status = ArrayDeque10::new();
    let mut in_bad_status = false;

    loop {
        let key = ctx.open_key_writable(url.as_str());

        let status = match surf::get(&url).timeout(TIMEOUT).await {
            Ok(response) => {
                let status = response.status();

                let string = status.to_string();
                if key.is_empty() { break }
                let _ = key.write(&string);

                if status.is_success() {
                    Healthy
                } else {
                    Unhealthy
                }
            },
            Err(e) => {
                let string = e.to_string();
                if key.is_empty() { break }
                let _ = key.write(&string);

                Unreacheable
            },
        };

        drop(key);

        last_status.push_front(status);

        let cap = last_status.capacity() as f32;
        let bads = last_status.iter().filter(|s| !s.is_good()).count() as f32;
        let ratio = bads / cap;

        if ratio >= 0.5 && !in_bad_status {
            in_bad_status = true;
            let report = (url.clone(), ReportStatus::Unhealthy);
            let _ = sender.send(report).await;
        }

        if ratio == 0.0 && in_bad_status {
            in_bad_status = false;
            let report = (url.clone(), ReportStatus::Healthy);
            let _ = sender.send(report).await;
        }

        let text_status = if in_bad_status { "bad" } else { "good" };
        eprintln!("{}/{} = {} (in {} status)", bads, cap, ratio, text_status);

        // in bad status
        // or last status is bad
        // or half of the status are bad
        // and this outage is "recent"
        if (in_bad_status || !status.is_good() || ratio >= 0.5) && ratio != 1.0 {
            let _ = Delay::new(FAST_PING).await;
        } else {
            let _ = Delay::new(NORMAL_PING).await;
        }
    }
}
