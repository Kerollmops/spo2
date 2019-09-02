#[macro_use] extern crate redismodule;

use redismodule::{parse_integer, Context, RedisError, RedisResult};

fn hello_mul(_: &Context, args: Vec<String>) -> RedisResult {
    if args.len() < 2 {
        return Err(RedisError::WrongArity);
    }

    let nums = args
        .into_iter()
        .skip(1)
        .map(|s| parse_integer(&s))
        .collect::<Result<Vec<i64>, RedisError>>()?;

    let product = nums.iter().product();

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
