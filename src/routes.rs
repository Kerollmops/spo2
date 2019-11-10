use std::io::{Cursor, Empty};
use std::{str, fmt};
use std::str::FromStr;

use url::Url;
use serde_json::Value;
use tiny_http::{Request, Response, Header};

use crate::health_checker::health_checker;
use crate::State;
use crate::url_value::UrlValue;
use crate::url_value::Status::{Healthy, Removed};

type BoxError = Box<dyn std::error::Error>;

pub fn into_json(data: Vec<u8>) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(data)
        .with_header(Header::from_str("Content-Type: application/json").unwrap())
}

pub fn into_internal_error<E: fmt::Display>(e: E) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(e.to_string())
        .with_status_code(500)
}

pub fn into_bad_request<E: fmt::Display>(e: E) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(e.to_string())
        .with_status_code(400)
}

pub fn not_found() -> Response<Empty> {
    Response::empty(404)
}

fn is_valid_url(url: &Url) -> bool {
    if url.cannot_be_a_base() {
        return false
    }

    match url.scheme() {
        "http" | "https" => true,
        _                => false,
    }
}

pub fn update_url(url: Url, mut request: Request, state: &State) -> Result<(), BoxError> {
    let url = match url.query_pairs().find(|(k, _)| k == "url") {
        Some((_, url)) => match Url::parse(&url) {
            Ok(url) => url,
            Err(e) => return request.respond(into_bad_request(e)).map_err(Into::into),
        },
        None => {
            return request.respond(into_bad_request("missing url parameter")).map_err(Into::into)
        },
    };

    if !is_valid_url(&url) {
        return request.respond(into_bad_request("Invalid url, must be an http/s url")).map_err(Into::into)
    }

    let mut body = String::new();
    if let Err(e) = request.as_reader().read_to_string(&mut body) {
        return request.respond(into_bad_request(e)).map_err(Into::into)
    }

    let user_data = if body.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice(body.as_bytes()) {
            Ok(value) => value,
            Err(e) => return request.respond(into_bad_request(e)).map_err(Into::into),
        }
    };

    let mut value = UrlValue {
        url: None,
        status: Healthy,
        reason: String::new(),
        data: user_data.clone(),
    };
    let mut value_bytes = match serde_json::to_vec(&value) {
        Ok(value_bytes) => value_bytes,
        Err(e) => return request.respond(into_internal_error(e)).map_err(Into::into),
    };

    let pool = &state.runtime;
    let database = state.database.clone();
    let notifier_sender = state.notifier_sender.clone();
    let event_sender = state.event_sender.clone();

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
            let message = match serde_json::to_string(&value) {
                Ok(message) => message,
                Err(e) => return request.respond(into_internal_error(e)).map_err(Into::into),
            };
            let _ = event_sender.send(message);

            pool.spawn_ok(async {
                health_checker(url, notifier_sender, event_sender, database).await
            });

            request.respond(into_json(value_bytes)).map_err(Into::into)
        },
        Ok(Some(_)) => request.respond(into_json(value_bytes)).map_err(Into::into),
        Err(e) => Err(e.into()),
    }
}

pub fn read_url(url: Url, request: Request, state: &State) -> Result<(), BoxError> {
    let url = match url.query_pairs().find(|(k, _)| k == "url") {
        Some((_, url)) => match Url::parse(&url) {
            Ok(url) => url,
            Err(e) => return request.respond(into_bad_request(e)).map_err(Into::into),
        },
        None => {
            return request.respond(into_bad_request("missing url parameter")).map_err(Into::into)
        },
    };

    let database = &state.database;
    match database.get(url.as_str()) {
        Ok(Some(value)) => request.respond(into_json(value.to_vec())).map_err(Into::into),
        Ok(None) => request.respond(not_found()).map_err(Into::into),
        Err(e) => request.respond(into_internal_error(e)).map_err(Into::into),
    }
}

pub fn delete_url(url: Url, request: Request, state: &State) -> Result<(), BoxError> {
    let url = match url.query_pairs().find(|(k, _)| k == "url") {
        Some((_, url)) => match Url::parse(&url) {
            Ok(url) => url,
            Err(e) => return request.respond(into_bad_request(e)).map_err(Into::into),
        },
        None => {
            return request.respond(into_bad_request("missing url parameter")).map_err(Into::into)
        },
    };

    let database = &state.database;
    let event_sender = &state.event_sender;

    match database.remove(url.as_str()) {
        Ok(Some(value_bytes)) => {
            let mut value: UrlValue = match serde_json::from_slice(&value_bytes) {
                Ok(value) => value,
                Err(e) => return request.respond(into_internal_error(e)).map_err(Into::into),
            };
            value.status = Removed;
            value.url = Some(url.to_string());

            let message = match serde_json::to_string(&value) {
                Ok(message) => message,
                Err(e) => return request.respond(into_internal_error(e)).map_err(Into::into),
            };
            let _ = event_sender.send(message);

            request.respond(into_json(value_bytes.to_vec())).map_err(Into::into)
        },
        Ok(None) => request.respond(not_found()).map_err(Into::into),
        Err(e) => request.respond(into_internal_error(e)).map_err(Into::into),
    }
}

pub fn get_all_urls(_url: Url, request: Request, state: &State) -> Result<(), BoxError> {
    let database = &state.database;

    let mut urls = Vec::new();
    for result in database.iter() {
        let (key, value) = match result {
            Ok(pair) => pair,
            Err(e) => return request.respond(into_internal_error(e)).map_err(Into::into),
        };

        let string = match str::from_utf8(&key) {
            Ok(string) => string,
            Err(e) => return request.respond(into_internal_error(e)).map_err(Into::into),
        };
        let url = match Url::parse(&string) {
            Ok(url) => url,
            Err(e) => return request.respond(into_internal_error(e)).map_err(Into::into),
        };

        let mut value: UrlValue = match serde_json::from_slice(&value) {
            Ok(value) => value,
            Err(e) => return request.respond(into_internal_error(e)).map_err(Into::into),
        };
        value.url = Some(url.to_string());

        urls.push(value);
    }

    let urls = match serde_json::to_vec(&urls) {
        Ok(urls) => urls,
        Err(e) => return request.respond(into_internal_error(e)).map_err(Into::into),
    };

    request.respond(into_json(urls)).map_err(Into::into)
}
