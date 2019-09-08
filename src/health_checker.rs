use std::time::Duration;

use futures::stream::StreamExt;
use futures_timer::{Interval, TryFutureExt};

use redismodule::ThreadSafeContext;
use url::Url;

const TIMEOUT: Duration = Duration::from_secs(5);

pub async fn health_checker(url: Url) {
    let ctx = ThreadSafeContext::create();
    let dur = Duration::from_secs(10);
    let mut stream = Interval::new(dur);

    while let Some(_) = stream.next().await {
        let key = ctx.open_key_writable(url.as_str());

        match surf::get(&url).timeout(TIMEOUT).await {
            Ok(response) => {
                let status = response.status();

                let string = status.to_string();
                if key.is_empty() { break }
                let _ = key.write(&string);

                if !status.is_success() {
                    eprintln!("{} {}", url, status);
                }
            },
            Err(error) => eprintln!("{} {}", url, error),
        }
    }
}
