mod health_checker;
mod routes;
mod url_value;

use std::cmp::Reverse;
use std::fmt::Write;
use std::str::FromStr;
use std::time::Duration;
use std::{env, io, str, thread};

use futures::channel::mpsc::{self, Sender};
use futures::executor::ThreadPool;
use futures::stream::StreamExt;
use futures_stream_batch::ChunksTimeoutStreamExt;
use isahc::prelude::*;
use subslice::SubsliceExt;
use tiny_http::{Response, Method, Header};
use url::Url;

use self::health_checker::health_checker;
use self::routes::{update_url, read_url, delete_url, get_all_urls};
use self::url_value::Report;

const HTTP_LISTEN_ADDR: &str = "HTTP_LISTEN_ADDR";
const WS_LISTEN_ADDR: &str = "WS_LISTEN_ADDR";
const SLACK_HOOK_URL: &str = "SLACK_HOOK_URL";
const DATABASE_PATH: &str = "DATABASE_PATH";

const HTML_CONTENT: &str = include_str!("../public/index.html");

pub struct State {
    runtime: ThreadPool,
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

    let runtime = ThreadPool::new().unwrap();
    let (notifier_sender, receiver) = mpsc::channel(100);
    let database = sled::Db::open(database_path).unwrap();

    // initialize the notifier sender
    runtime.spawn_ok(async move {
        let slack_hook_url = match env::var(SLACK_HOOK_URL) {
            Ok(url) => url,
            Err(e) => {
                eprintln!("{}: {}", SLACK_HOOK_URL, e);
                return
            }
        };

        let mut receiver = receiver.chunks_timeout(40, Duration::new(10, 0));
        while let Some(mut reports) = receiver.next().await {
            // remove subsequent urls status for the same url
            reports.sort_by_key(|r: &Report| Reverse(r.url.clone()));
            reports.dedup_by_key(|r| r.url.clone());

            let mut body = String::new();

            // if reports contain newly detected bad status
            if reports.iter().any(|r| !r.still && !r.status.is_good()) {
                let _ = writeln!(&mut body, "<!channel>");
            }

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
            let request = Request::post(&slack_hook_url)
                .header("content-type", "application/json")
                .body(serde_json::to_vec(&body).unwrap())
                .unwrap();

            if let Err(e) = isahc::send_async(request).await {
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

        runtime.spawn_ok(async {
            health_checker(url, notifier_sender, event_sender, database).await
        });
    }

    let state = State { runtime, notifier_sender, event_sender, database };

    let server = tiny_http::Server::http(http_listen_addr).unwrap();
    let http_listen_addr = server.server_addr();
    eprintln!("Listening on {}", http_listen_addr);

    let base_url = format!("http://{}", http_listen_addr);
    let base_url = Url::parse(&base_url).unwrap();

    loop {
        let request = match server.recv() {
            Ok(request) => request,
            Err(e) => { eprintln!("{}", e); continue }
        };

        let method = request.method();
        let url = match base_url.join(request.url()) {
            Ok(url) => url,
            Err(error) => {
                let message = error.to_string();
                let response = Response::from_string(message).with_status_code(400);
                if let Err(e) = request.respond(response) { eprintln!("{}", e) }
                continue;
            }
        };

        fn accept_text_html(header: &Header) -> bool {
            header.field.equiv("accept") && header.value.as_bytes().find(b"text/html").is_some()
        }

        let result = match (url.path(), method) {
            ("/all", &Method::Get) => get_all_urls(url, request, &state),
            ("/", &Method::Get) => {
                if request.headers().iter().any(accept_text_html) {
                    let response = Response::from_string(HTML_CONTENT)
                        .with_header(Header::from_str("Content-Type: text/html; charset=utf-8").unwrap())
                        .with_status_code(200);
                    request.respond(response).map_err(Into::into)
                } else {
                    read_url(url, request, &state)
                }
            },
            ("/", &Method::Post) => update_url(url, request, &state),
            ("/", &Method::Put) => update_url(url, request, &state),
            ("/", &Method::Delete) => delete_url(url, request, &state),
            (_path, _method) => request.respond(Response::empty(404)).map_err(Into::into),
        };

        if let Err(e) = result {
            eprintln!("{}", e);
        }
    }
}
