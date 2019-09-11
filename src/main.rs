mod health_checker;
mod response;
mod routes;

use std::{env, io};

use futures::channel::mpsc::{self, Sender};
use futures::executor::ThreadPool;
use futures::stream::StreamExt;
use url::Url;

use self::routes::{update_url, read_url, delete_url};

const SLACK_HOOK_URL: &str = "SLACK_HOOK_URL";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReportStatus {
    Unhealthy,
    Healthy,
}

pub struct State {
    thread_pool: ThreadPool,
    notifier_sender: Sender<(Url, ReportStatus)>,
    database: sled::Db,
}

fn main() -> Result<(), io::Error> {
    let thread_pool = ThreadPool::new().unwrap();
    let (notifier_sender, receiver) = mpsc::channel(100);
    let database = sled::Db::open("spo2.db").unwrap();

    // initialize the notifier sender
    thread_pool.spawn_ok(async move {
        let slack_hook_url = match env::var(SLACK_HOOK_URL) {
            Ok(url) => url,
            Err(e) => { eprintln!("SLACK_HOOK_URL: {}", e); return }
        };

        let mut receiver = receiver;
        while let Some((url, status)) = receiver.next().await {
            let body = format!("{} reported {:?}", url, status);
            let body = serde_json::json!({ "text": body });
            let request = surf::post(&slack_hook_url).body_json(&body).unwrap();
            if let Err(e) = request.recv_string().await {
                eprintln!("{}", e);
            }
        }
    });

    let state = State {
        thread_pool: thread_pool.clone(),
        notifier_sender: notifier_sender.clone(),
        database: database.clone(),
    };

    let mut app = tide::App::with_state(state);

    app.at("/:url")
        .post(update_url)
        .get(read_url)
        .put(update_url)
        .delete(delete_url);

    let listen_addr = env::args().nth(1).unwrap_or("127.0.0.1:8000".into());

    app.run(listen_addr)
}
