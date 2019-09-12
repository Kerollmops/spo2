use std::time::{Duration, Instant};

use futures::channel::mpsc::Sender;
use futures::sink::SinkExt;
use futures_timer::{Delay, TryFutureExt};
use url::Url;

use crate::url_value::{Status, UrlValue};
use crate::url_value::Status::{Healthy, Unhealthy, Unreacheable};

const STILL_UNHEALTHY_TIMEOUT: Duration = Duration::from_secs(15 * 60); // 15 minutes
const TIMEOUT:     Duration = Duration::from_secs(5);
const NORMAL_PING: Duration = Duration::from_secs(3);
const FAST_PING:   Duration = Duration::from_millis(800);

type ArrayDeque10<T> = arraydeque::ArrayDeque<[T; 10], arraydeque::Wrapping>;

pub async fn health_checker(
    url: Url,
    mut report_sender: Sender<(Url, Status)>,
    event_sender: ws::Sender,
    database: sled::Db,
)
{
    let mut last_status = ArrayDeque10::new();
    let mut in_bad_state_since = None;

    loop {
        let (status, reason) = match surf::get(&url).timeout(TIMEOUT).await {
            Ok(ref resp) if resp.status().is_success() => {
                (Healthy, resp.status().to_string())
            },
            Ok(resp) => (Unhealthy, resp.status().to_string()),
            Err(e) => (Unreacheable, e.to_string()),
        };

        last_status.push_front(status);

        // update this value but do not erase the user custom data updates
        let result = database.update_and_fetch(url.as_str(), |old| {
            let old = old?;
            let mut value: UrlValue = serde_json::from_slice(old).unwrap();
            value.status = status;
            value.reason = reason.clone();
            Some(serde_json::to_vec(&value).unwrap())
        });

        // retrieve the new value and deserialize it
        // assign the current url this way it can be send in notifications
        let value = match result {
            Ok(Some(value)) => {
                let mut value: UrlValue = serde_json::from_slice(&value).unwrap();
                value.url = Some(url.to_string());
                value
            },
            Ok(None) => break,
            Err(e) => { eprintln!("{}: {}", url, e); return },
        };

        let cap = last_status.capacity() as f32;
        let bads = last_status.iter().filter(|s| !s.is_good()).count() as f32;
        let ratio = bads / cap;

        if ratio >= 0.5 && in_bad_state_since.is_none() {
            in_bad_state_since = Some(Instant::now());

            let report = (url.clone(), Status::Unhealthy);
            let _ = report_sender.send(report).await;

            let message = serde_json::to_string(&value).unwrap();
            let _ = event_sender.send(message);
        }

        if ratio == 0.0 && in_bad_state_since.is_some() {
            in_bad_state_since = None;

            let report = (url.clone(), Status::Healthy);
            let _ = report_sender.send(report).await;

            let message = serde_json::to_string(&value).unwrap();
            let _ = event_sender.send(message);
        }

        if (in_bad_state_since.is_some() || !status.is_good() || ratio >= 0.5) && ratio != 1.0 {
            let _ = Delay::new(FAST_PING).await;
        } else {
            let _ = Delay::new(NORMAL_PING).await;
        }

        if let Some(since) = in_bad_state_since {
            if since.elapsed() > STILL_UNHEALTHY_TIMEOUT {
                let report = (url.clone(), Status::Unhealthy);
                let _ = report_sender.send(report).await;

                let message = serde_json::to_string(&value).unwrap();
                let _ = event_sender.send(message);

                in_bad_state_since = Some(Instant::now());
            }
        }
    }
}
