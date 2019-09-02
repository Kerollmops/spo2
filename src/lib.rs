#[macro_use] extern crate redismodule;

use std::time::Duration;
use futures_timer::Interval;
use futures::prelude::*;
use redismodule::{parse_integer, Context, RedisError, RedisResult};

fn spawn_future<F>(fut: F)
where F: Future<Output = ()> + Send + 'static
{
    use runtime_raw::Runtime;
    use runtime_native::Native;
    Native.spawn_boxed(fut.boxed()).expect("cannot spawn a future");
}

fn hello_mul(_context: &Context, args: Vec<String>) -> RedisResult {
    if args.len() < 2 {
        return Err(RedisError::WrongArity);
    }

    let nums = args
        .into_iter()
        .skip(1)
        .map(|s| parse_integer(&s))
        .collect::<Result<Vec<i64>, RedisError>>()?;

    let product = nums.iter().product();

    spawn_future(async move {
        let dur = Duration::from_secs(4);
        let mut stream = Interval::new(dur);

        while let Some(_) = stream.next().await {
            let url = format!("https://httpbin.org/status/{}", product);
            match surf::get(url).await {
                Ok(response) => println!("{:?}", response),
                Err(error) => println!("{}", error),
            }
        }
    });

    let mut response = Vec::from(nums);
    response.push(product);

    return Ok(response.into());
}

redis_module! {
    name: "hello",
    version: 1,
    data_types: [],
    commands: [
        ["hello.mul", hello_mul, ""],
    ],
}
