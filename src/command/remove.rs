use futures::{executor::block_on, compat::Compat01As03};
use redismodule::{Context, RedisError, RedisResult};
use url::Url;
use crate::SCAN_LOCK;

pub fn spo2_remove(ctx: &Context, args: Vec<String>) -> RedisResult {
    let key = match args.as_slice() {
        [_, key] => key,
        _ => return Err(RedisError::WrongArity),
    };

    // TODO it would be better to use RedisModule_Un/BlockClient
    block_on(async move {
        let lock = SCAN_LOCK.get().expect("scan lock uninitialized");
        let lock = Compat01As03::new(lock.read()).await;
        drop(lock);
    });

    match Url::parse(key).map(Url::into_string) {
        Ok(url) => ctx.call("DEL", &[&url]),
        Err(e) => Err(RedisError::String(format!("{:?}: {}", key, e))),
    }
}
