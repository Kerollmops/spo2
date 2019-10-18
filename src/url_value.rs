use serde::{Serialize, Deserialize};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    Healthy,
    Unhealthy,
    Unreacheable,
    Removed,
}

impl Status {
    pub fn is_good(&self) -> bool {
        *self == Status::Healthy
    }
}

pub struct Report {
    pub url: Url,
    pub status: Status,
    pub still: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    pub status: Status,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,

    /// This is the client custom data
    pub data: serde_json::Value,
}
