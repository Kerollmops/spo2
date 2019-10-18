mod either_response;
mod health_checker;
mod response;
mod routes;
mod url_value;

use std::fmt::Write;
use std::time::Duration;
use std::{env, io, str, thread};

use futures::channel::mpsc::{self, Sender};
use futures::stream::StreamExt;
use subslice::SubsliceExt;
use tide::Context;
use tide::http::header::HeaderValue;
use tide::middleware::{CorsMiddleware, CorsOrigin};
use tokio::runtime::Runtime;
use tokio_batch::ChunksTimeoutStreamExt;
use url::Url;

use self::either_response::Either;
use self::health_checker::health_checker;
use self::routes::{update_url, read_url, delete_url, get_all_urls};
use self::url_value::Report;

const HTTP_LISTEN_ADDR: &str = "HTTP_LISTEN_ADDR";
const WS_LISTEN_ADDR: &str = "WS_LISTEN_ADDR";
const SLACK_HOOK_URL: &str = "SLACK_HOOK_URL";
const DATABASE_PATH: &str = "DATABASE_PATH";

const HTML_CONTENT: &str = include_str!("../public/index.html");

pub struct State {
    runtime: Runtime,
    notifier_sender: Sender<Report>,
    event_sender: ws::Sender,
    database: sled::Db,
}

fn main() -> Result<(), io::Error> {
    let ws_listen_addr = match env::var(WS_LISTEN_ADDR) {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("{}: {}", WS_LISTEN_ADDR, e);
            String::from("127.0.0.1:8001")
        },
    };

    let http_listen_addr = match env::var(HTTP_LISTEN_ADDR) {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("{}: {}", HTTP_LISTEN_ADDR, e);
            String::from("127.0.0.1:8000")
        },
    };

    let database_path = match env::var(DATABASE_PATH) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("{}: {}", DATABASE_PATH, e);
            String::from("spo2.db")
        },
    };

    let runtime = Runtime::new().unwrap();
    let (notifier_sender, receiver) = mpsc::channel(100);
    let database = sled::Db::open(database_path).unwrap();

    // initialize the notifier sender
    runtime.spawn(async move {
        let slack_hook_url = match env::var(SLACK_HOOK_URL) {
            Ok(url) => url,
            Err(e) => {
                eprintln!("{}: {}", SLACK_HOOK_URL, e);
                return
            }
        };

        let mut receiver = receiver.chunks_timeout(40, Duration::new(10, 0));
        while let Some(reports) = receiver.next().await {

            println!("reports: {:?}", reports);

            let mut body = String::new();
            for Report { url, status, still, reason } in reports {
                let _ = if still {
                    writeln!(&mut body, "{} is still {:?}", url, status)
                } else if status.is_good() {
                    writeln!(&mut body, "{} is now {:?} 🎉", url, status)
                } else {
                    writeln!(&mut body, "{} reported {:?} ({})", url, status, reason)
                };
            }

            let body = serde_json::json!({ "text": body });
            let client = reqwest::Client::new();
            if let Err(e) = client.post(slack_hook_url.as_str()).json(&body).send().await {
                eprintln!("{}", e);
            }
        }
    });

    // initialize the websocket listener
    let builder = ws::Builder::new();
    let ws = builder.build(|_| |_| Ok(())).unwrap();
    let event_sender = ws.broadcaster();

    // run the websocket listener
    println!("Websocket server is listening on: ws://{}", ws_listen_addr);
    let _ = thread::spawn(|| {
        ws.listen(ws_listen_addr).expect("websocket listen error")
    });

    // run health checking for every url saved
    for result in database.iter() {
        let key = match result {
            Ok((key, _)) => key,
            Err(e) => { eprintln!("{}", e); continue },
        };

        let string = str::from_utf8(&key).unwrap();
        let url = Url::parse(string).unwrap();

        let notifier_sender = notifier_sender.clone();
        let database = database.clone();
        let event_sender = event_sender.clone();

        runtime.spawn(async {
            health_checker(url, notifier_sender, event_sender, database).await
        });
    }

    let state = State { runtime, notifier_sender, event_sender, database };
    let mut app = tide::App::with_state(state);

    app.middleware(
        CorsMiddleware::new()
            .allow_origin(CorsOrigin::from("*"))
            .allow_methods(HeaderValue::from_static("GET, POST, DELETE, OPTIONS")),
    );

    app.at("/")
        .get(|cx: Context<State>| async move {
            if cx.headers().get("Accept").and_then(|v| v.as_bytes().find(b"text/html")).is_some() {
                Either::Left(tide::http::Response::builder()
                    .header(tide::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .status(tide::http::StatusCode::OK)
                    .body(HTML_CONTENT).unwrap())
            } else {
                Either::Right(read_url(cx).await)
            }
        })
        .post(update_url)
        .put(update_url)
        .delete(delete_url);

    app.at("/all")
        .get(get_all_urls);

    // start listening to clients now
    app.run(http_listen_addr)
}
