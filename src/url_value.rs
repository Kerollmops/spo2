use std::time::Instant;
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

#[derive(Debug)]
pub struct Report {
    pub url: Url,
    pub status: Status,
    pub since: Option<Instant>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlValue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    pub status: Status,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,

    #[serde(default = "Instant::now", with = "approx_instant")]
    pub since: Instant,

    /// This is the client custom data
    pub data: serde_json::Value,
}

pub mod approx_instant {
    use std::time::{Instant, SystemTime};
    use serde::{Serialize, Serializer, Deserialize, Deserializer, de::Error};

    pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let system_now = SystemTime::now();
        let instant_now = Instant::now();
        let approx = system_now - (instant_now - *instant);
        approx.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
    where
        D: Deserializer<'de>,
    {
        let de = SystemTime::deserialize(deserializer)?;
        let system_now = SystemTime::now();
        let instant_now = Instant::now();
        let duration = system_now.duration_since(de).map_err(Error::custom)?;
        let approx = instant_now - duration;
        Ok(approx)
    }
}
