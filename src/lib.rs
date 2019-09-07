#[macro_use] extern crate redismodule;

mod command;
mod health_checker;

use std::ffi::CStr;
use std::time::Duration;

use futures::executor::ThreadPool;
use futures::compat::Compat01As03;
use futures_locks::RwLock;
use once_cell::sync::OnceCell;
use redismodule::{Context, ThreadSafeContext, RedisError};
use url::Url;

use self::command::{spo2_insert, spo2_remove};
use self::health_checker::health_checker;

static THREAP_POOL: OnceCell<ThreadPool> = OnceCell::new();
static SCAN_LOCK: OnceCell<RwLock<()>> = OnceCell::new();

unsafe extern "C" fn event_subscription(
    _ctx: *mut raw::RedisModuleCtx,
    _type_: i32,
    event: *const i8,
    key: *mut raw::RedisModuleString,
) -> i32
{
    let event = CStr::from_ptr(event);
    println!("{:?}", event);
    if event.to_bytes() != b"set" { return 0 }

    let pool = THREAP_POOL.get().expect("global thread pool uninitialized");

    let key = match RedisString::from_ptr(key) {
        Ok(key) => key,
        Err(e) => { eprintln!("{:?}: {}", key, e); return 1 },
    };

    println!("{:?} {}", event, key);

    let url = match Url::parse(key) {
        Ok(url) => url,
        Err(e) => { eprintln!("{:?}: {}", key, e); return 1 },
    };

    pool.spawn_ok(async { health_checker(url).await });

    0
}

fn init_function(_ctx: *mut raw::RedisModuleCtx) -> i32 {
    let pool = THREAP_POOL.get_or_try_init(ThreadPool::new).unwrap();
    let lock = SCAN_LOCK.get_or_init(|| RwLock::new(()));

    pool.spawn_ok(async move {
        // write lock the init mutex
        let lock = Compat01As03::new(lock.write()).await;
        let ctx = ThreadSafeContext::create();

        // REDISMODULE_NOTIFY_SET does not work...
        let types = raw::REDISMODULE_NOTIFY_ALL as i32;
        let ret = ctx.subscribe_to_keyspace_events(types, Some(event_subscription));
        assert_eq!(ret, 0);

        // SCAN all keys
        //   GETSET $key ''

        println!("doing key scan...");
        std::thread::sleep(Duration::from_secs(2));
        println!("key scan done.");

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
