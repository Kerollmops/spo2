#[macro_use] extern crate redismodule;

mod command;
mod health_checker;

use std::ffi::CStr;

use futures::executor::ThreadPool;
use futures::compat::Compat01As03;
use futures_locks::RwLock;
use once_cell::sync::Lazy;
use redismodule::{Context, ThreadSafeContext, RedisError, RedisValue};
use url::Url;

use self::command::{spo2_insert, spo2_remove};
use self::health_checker::health_checker;

static THREAP_POOL: Lazy<ThreadPool> = Lazy::new(|| ThreadPool::new().unwrap());
static SCAN_LOCK: Lazy<RwLock<()>> = Lazy::new(|| RwLock::new(()));

unsafe extern "C" fn event_subscription(
    _ctx: *mut raw::RedisModuleCtx,
    _type_: i32,
    event: *const i8,
    key: *mut raw::RedisModuleString,
) -> i32
{
    let event = CStr::from_ptr(event);
    if event.to_bytes() != b"set" { return 0 }

    let key = match RedisString::from_ptr(key) {
        Ok(key) => key,
        Err(e) => { eprintln!("{:?}: {}", key, e); return 1 },
    };

    let url = match Url::parse(key) {
        Ok(url) => url,
        Err(e) => { eprintln!("{:?}: {}", key, e); return 1 },
    };

    THREAP_POOL.spawn_ok(async { health_checker(url).await });

    0
}

fn init_function(_ctx: *mut raw::RedisModuleCtx) -> i32 {
    THREAP_POOL.spawn_ok(async move {
        // write lock the init mutex
        let lock = Compat01As03::new(SCAN_LOCK.write()).await;
        let ctx = ThreadSafeContext::create();

        // REDISMODULE_NOTIFY_SET does not work...
        let types = raw::REDISMODULE_NOTIFY_ALL as i32;
        let ret = ctx.subscribe_to_keyspace_events(types, Some(event_subscription));
        assert_eq!(ret, 0);

        let mut cursor: usize = 0;
        loop {
            let arg = cursor.to_string();
            let result = ctx.call("scan", &[&arg]);

            let mut args = match result {
                Ok(RedisValue::Array(array)) => array.into_iter(),
                Ok(_) => break,
                Err(e) => { eprintln!("{:?}", e); break },
            };

            match args.next() {
                Some(RedisValue::SimpleString(string)) => {
                    cursor = string.parse().unwrap()
                },
                _ => panic!("wooops"),
            }

            let keys = match args.next() {
                Some(RedisValue::Array(array)) => {
                    array
                        .into_iter()
                        .filter_map(|e| match e {
                            RedisValue::SimpleString(string) => Some(string),
                            _ => None,
                        })
                },
                _ => panic!("wooops"),
            };

            for key in keys {
                if let Err(e) = ctx.call("GETSET", &[&key, ""]) {
                    panic!("{:?}", e);
                }
            }

            if cursor == 0 { break }
        }

        // unlock the init mutex this way
        // insert/remove commands can continue
        drop(lock);
    });

    0
}

redis_module! {
    name: "SpO2",
    version: 1,
    data_types: [],
    init: init_function,
    commands: [
        ["spo2.insert", spo2_insert, "write"],
        ["spo2.remove", spo2_remove, "write"],
    ],
}
