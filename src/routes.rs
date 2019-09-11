use tide::Context;
use tide::response::WithStatus;
use url::Url;

use crate::health_checker::health_checker;
use crate::response::{Json, into_internal_error, into_bad_request, not_found};
use crate::State;

pub async fn update_url(mut cx: Context<State>) -> Result<Json, WithStatus<String>> {
    let url = cx.param("url").map(urldecode::decode).unwrap();
    let url = Url::parse(&url).map_err(into_bad_request)?;

    let body =  cx.body_bytes().await.map_err(into_bad_request)?;
    let body = if body.is_empty() { None } else { Some(body) };

    let value = match body {
        None => serde_json::Value::Null,
        Some(body) => serde_json::from_slice(&body).map_err(into_bad_request)?,
    };

    let value = serde_json::json!({ "data": value });
    let value = serde_json::to_vec(&value).map_err(into_internal_error)?;

    let pool = &cx.state().thread_pool;
    let database = cx.state().database.clone();
    let notifier_sender = cx.state().notifier_sender.clone();
    let event_sender = cx.state().event_sender.clone();

    match database.insert(url.as_str(), value.as_slice()) {
        Ok(None) => {
            pool.spawn_ok(async {
                health_checker(url, notifier_sender, event_sender, database).await
            });
            Ok(Json(value))
        },
        Ok(Some(_)) => Ok(Json(value)),
        Err(e) => Err(into_internal_error(e)),
    }
}

pub async fn read_url(cx: Context<State>) -> Result<Json, WithStatus<String>> {
    let url = cx.param("url").map(urldecode::decode).unwrap();
    let url = Url::parse(&url).map_err(into_bad_request)?;

    let database = &cx.state().database;
    let event_sender = &cx.state().event_sender;

    let message = format!("{} Read", url);
    event_sender.send(message).unwrap();

    match database.get(url.as_str()) {
        Ok(Some(value)) => Ok(Json(value.to_vec())),
        Ok(None) => Err(not_found()),
        Err(e) => Err(into_internal_error(e)),
    }
}

pub async fn delete_url(cx: Context<State>) -> Result<Json, WithStatus<String>> {
    let url = cx.param("url").map(urldecode::decode).unwrap();
    let url = Url::parse(&url).map_err(into_bad_request)?;

    let database = &cx.state().database;
    let event_sender = &cx.state().event_sender;

    match database.remove(url.as_str()) {
        Ok(Some(value)) => Ok(Json(value.to_vec())),
        Ok(None) => Err(not_found()),
        Err(e) => Err(into_internal_error(e)),
    }
}
