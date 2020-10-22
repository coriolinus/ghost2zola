//! The Ghost data model
//!
//! <https://ghost.org/docs/api/v3/migration/developers/>
//!
//! When deserializing unknown data, deserialize it into `Top`, which handles the optional DB wrapper.

use chrono::{
    serde::{ts_milliseconds, ts_milliseconds_option},
    DateTime, Utc,
};
use mobiledoc::Mobiledoc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Top {
    Wrapper(Wrapper),
    Db(Db),
}

impl Top {
    pub fn dbs<'a>(&'a self) -> Box<dyn 'a + Iterator<Item = &Db>> {
        match self {
            Self::Db(db) => Box::new(std::iter::once(db)),
            Self::Wrapper(wrapper) => Box::new(wrapper.db.iter()),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Wrapper {
    pub db: Vec<Db>,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Db {
    pub meta: Meta,
    pub data: Data,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct Meta {
    #[serde(with = "ts_milliseconds")]
    pub exported_on: DateTime<Utc>,
    pub version: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Data {
    pub posts: Vec<Value>,
    pub tags: Vec<Value>,
    pub users: Vec<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posts_tags: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posts_authors: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roles_authors: Option<Vec<Value>>,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Post {
    pub title: String,
    #[serde(with = "mobiledoc::serde_str_option")]
    pub mobiledoc: Option<Mobiledoc>,
    pub status: Option<String>,
    #[serde(with = "ts_milliseconds_option")]
    pub published_at: Option<DateTime<Utc>>,
}
