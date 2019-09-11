use std::time::Duration;

use futures::sink::SinkExt;
use futures::channel::mpsc::Sender;
use futures_timer::{Delay, TryFutureExt};

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

pub async fn health_checker(
    url: Url,
    mut report_sender: Sender<(Url, ReportStatus)>,
    event_sender: ws::Sender,
    database: sled::Db,
)
{
    let mut last_status = ArrayDeque10::new();
    let mut in_bad_status = false;

    let message = format!("{},{:?}", url, ReportStatus::Healthy);
    let _ = event_sender.send(message);

    loop {
        let status = match surf::get(&url).timeout(TIMEOUT).await {
            Ok(ref resp) if resp.status().is_success() => Healthy,
            Ok(resp) => Unhealthy,
            Err(e) => Unreacheable,
        };

        last_status.push_front(status);

        match database.get(url.as_str()) {
            Ok(Some(_)) => (),
            Ok(None) => break,
            Err(e) => eprintln!("{}: {}", url, e),
        }

        let cap = last_status.capacity() as f32;
        let bads = last_status.iter().filter(|s| !s.is_good()).count() as f32;
        let ratio = bads / cap;

        if ratio >= 0.5 && !in_bad_status {
            in_bad_status = true;

            let report = (url.clone(), ReportStatus::Unhealthy);
            let _ = report_sender.send(report).await;

            let message = format!("{},{:?}", url, ReportStatus::Unhealthy);
            let _ = event_sender.send(message);
        }

        if ratio == 0.0 && in_bad_status {
            in_bad_status = false;

            let report = (url.clone(), ReportStatus::Healthy);
            let _ = report_sender.send(report).await;

            let message = format!("{},{:?}", url, ReportStatus::Healthy);
            let _ = event_sender.send(message);
        }

        if (in_bad_status || !status.is_good() || ratio >= 0.5) && ratio != 1.0 {
            let _ = Delay::new(FAST_PING).await;
        } else {
            let _ = Delay::new(NORMAL_PING).await;
        }
    }

    let message = format!("{},{}", url, "Removed");
    let _ = event_sender.send(message);
}
