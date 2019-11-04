use std::str;

use tide::Context;
use tide::querystring::ContextExt;
use tide::response::WithStatus;
use url::Url;
use serde::Deserialize;
use serde_json::Value;

use crate::health_checker::health_checker;
use crate::response::{Json, into_internal_error, into_bad_request, not_found};
use crate::State;
use crate::url_value::UrlValue;
use crate::url_value::Status::{Healthy, Removed};

fn is_valid_url(url: &Url) -> bool {
    if url.cannot_be_a_base() {
        return false
    }

    match url.scheme() {
        "http" | "https" => true,
        _                => false,
    }
}

#[derive(Deserialize)]
struct QueryParams {
    url: Url,
}

pub async fn update_url(mut cx: Context<State>) -> Result<Json, WithStatus<String>> {
    let url = match cx.url_query::<QueryParams>() {
        Ok(qp) => qp.url,
        Err(_) => return Err(into_bad_request("Invalid query parameters")),
    };

    if !is_valid_url(&url) {
        return Err(into_bad_request("Invalid url, must be an http/s url"))
    }

    let body = cx.body_bytes().await.map_err(into_bad_request)?;
    let user_data = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).map_err(into_bad_request)?
    };

    let mut value = UrlValue {
        url: None,
        status: Healthy,
        reason: String::new(),
        data: user_data.clone(),
    };
    let mut value_bytes = serde_json::to_vec(&value).map_err(into_internal_error)?;

    let pool = &cx.state().runtime;
    let database = cx.state().database.clone();
    let notifier_sender = cx.state().notifier_sender.clone();
    let event_sender = cx.state().event_sender.clone();

    // update this value but do not erase
    // the last status written by the health checker
    let result = database.fetch_and_update(url.as_str(), |old| {
        match old {
            Some(old) => {
                value = serde_json::from_slice(old).unwrap();
                value.data = user_data.clone();
                value_bytes = serde_json::to_vec(&value).unwrap();
                Some(value_bytes.clone())
            },
            None => Some(value_bytes.clone()),
        }
    });

    match result {
        Ok(None) => {
            // send the initial healthy message when an url is added
            value.url = Some(url.to_string());
            let message = serde_json::to_string(&value).map_err(into_internal_error)?;
            let _ = event_sender.send(message);

            pool.spawn_ok(async {
                health_checker(url, notifier_sender, event_sender, database).await
            });

            Ok(Json(value_bytes))
        },
        Ok(Some(_)) => Ok(Json(value_bytes)),
        Err(e) => Err(into_internal_error(e)),
    }
}

pub async fn read_url(cx: Context<State>) -> Result<Json, WithStatus<String>> {
    let url = match cx.url_query::<QueryParams>() {
        Ok(qp) => qp.url,
        Err(_) => return Err(into_bad_request("Invalid query parameters")),
    };

    let database = &cx.state().database;
    match database.get(url.as_str()) {
        Ok(Some(value)) => Ok(Json(value.to_vec())),
        Ok(None) => Err(not_found()),
        Err(e) => Err(into_internal_error(e)),
    }
}

pub async fn delete_url(cx: Context<State>) -> Result<Json, WithStatus<String>> {
    let url = match cx.url_query::<QueryParams>() {
        Ok(qp) => qp.url,
        Err(_) => return Err(into_bad_request("Invalid query parameters")),
    };

    let database = &cx.state().database;
    let event_sender = &cx.state().event_sender;

    match database.remove(url.as_str()) {
        Ok(Some(value_bytes)) => {
            let mut value: UrlValue = serde_json::from_slice(&value_bytes).map_err(into_internal_error)?;
            value.status = Removed;
            value.url = Some(url.to_string());

            let message = serde_json::to_string(&value).map_err(into_internal_error)?;
            let _ = event_sender.send(message);

            Ok(Json(value_bytes.to_vec()))
        },
        Ok(None) => Err(not_found()),
        Err(e) => Err(into_internal_error(e)),
    }
}

pub async fn get_all_urls(cx: Context<State>) -> Result<Json, WithStatus<String>> {
    let database = &cx.state().database;

    let mut urls = Vec::new();
    for result in database.iter() {
        let (key, value) = match result {
            Ok(pair) => pair,
            Err(e) => return Err(into_internal_error(e)),
        };

        let string = str::from_utf8(&key).map_err(into_internal_error)?;
        let url = Url::parse(&string).map_err(into_internal_error)?;

        let mut value: UrlValue = serde_json::from_slice(&value).map_err(into_internal_error)?;
        value.url = Some(url.to_string());

        urls.push(value);
    }

    let urls = serde_json::to_vec(&urls).map_err(into_internal_error)?;
    Ok(Json(urls))
}
