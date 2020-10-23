use chrono::{DateTime, Utc};
use rusqlite::{
    self, params,
    types::{FromSql, FromSqlResult},
    Connection,
};
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Status {
    Published,
    Draft,
}

impl FromStr for Status {
    type Err = ();

    fn from_str(s: &str) -> Result<Status, Self::Err> {
        if s == "published" {
            Ok(Status::Published)
        } else {
            Ok(Status::Draft)
        }
    }
}

impl FromSql for Status {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        value
            .as_str()
            .map(|str| Status::from_str(str).expect("Status::from_str is infallible"))
    }
}

impl Status {
    pub fn draft(&self) -> bool {
        *self == Status::Draft
    }
}

#[derive(Debug)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub description: Option<String>,
    // Sqlite uses UTC for all times by default:
    // <https://sqlite.org/lang_datefunc.html> section 2
    pub date: Option<DateTime<Utc>>,
    pub updated: Option<DateTime<Utc>>,
    pub status: Option<Status>,
    pub slug: String,
    pub language: String,
    pub author_name: String,
}

impl Post {
    pub fn query(conn: &Connection) -> Result<Vec<Self>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "
            SELECT
                posts.id,
                posts.title,
                posts.markdown,
                posts.meta_description,
                posts.published_at,
                posts.updated_at,
                posts.status,
                posts.slug,
                posts.language,
                users.name
            FROM posts
            INNER JOIN users
            ON posts.author_id = users.id
            ",
        )?;
        let out = stmt
            .query_map(params![], |row| {
                Ok(Post {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(3)?,
                    description: row.get(4)?,
                    date: row.get(5)?,
                    updated: row.get(6)?,
                    status: row.get(7)?,
                    slug: row.get(8)?,
                    language: row.get(9)?,
                    author_name: row.get(10)?,
                })
            })?
            .collect();
        out
    }

    pub fn tags(&self, conn: &Connection) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "
            SELECT
                tags.name
            FROM tags
            INNER JOIN posts_tags
            ON tags.id = posts_tags.tag_id
            WHERE posts_tags.post_id = ?1
            ",
        )?;
        let out = stmt
            .query_map(params![self.id], |row| Ok(row.get(0)?))?
            .collect();
        out
    }
}
