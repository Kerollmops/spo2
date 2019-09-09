use std::time::Duration;

use futures::stream::StreamExt;
use futures_timer::{Interval, TryFutureExt};

use redismodule::ThreadSafeContext;
use url::Url;

const TIMEOUT:      Duration = Duration::from_secs(5);
const NORMAL_PING:  Duration = Duration::from_secs(3);
const FAST_PING:    Duration = Duration::from_millis(800);
const INSTANT_PING: Duration = Duration::from_millis(100);

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

pub async fn health_checker(url: Url) {
    let ctx = ThreadSafeContext::create();
    let mut last_status = ArrayDeque10::new();
    let mut stream = Interval::new(INSTANT_PING);
    let mut in_bad_status = false;

    while let Some(_) = stream.next().await {
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
            Err(_) => Unreacheable,
        };

        last_status.push_front(status);

        let len = last_status.len() as f32;
        let bads = last_status.iter().filter(|s| !s.is_good()).count() as f32;
        let ratio = bads / len;

        println!("{}/{} = {}", bads, len, ratio);

        // in bad status or last status is bad or half of the status are bad
        if in_bad_status || !status.is_good() || ratio >= 0.5 {
            eprintln!("{} speed up ping", url);
            stream = Interval::new(FAST_PING);
        } else {
            stream = Interval::new(NORMAL_PING);
        }

        if ratio >= 0.5 && !in_bad_status {
            in_bad_status = true;
            eprintln!("{} reported as bad", url);
        }

        if ratio == 0.0 && in_bad_status {
            in_bad_status = false;
            eprintln!("{} reported as good now", url);
        }

        drop(key);
    }
}
