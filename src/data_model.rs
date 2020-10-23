use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use rusqlite::{
    self, params,
    types::{FromSql, FromSqlResult},
    Connection,
};
use serde::Serialize;
use slugify::slugify;
use std::fmt;
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;

lazy_static! {
    static ref INTERNAL_LINK_RE: Regex =
        regex::RegexBuilder::new(r"\]\(/content/images/\d{4}/\d{2}/([^)]+)\)")
            .case_insensitive(true)
            .build()
            .unwrap();
}

pub(crate) fn relative_internal_links(text: &str) -> String {
    INTERNAL_LINK_RE.replace_all(text, "](../$1)").into_owned()
}

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

    pub fn published(&self) -> bool {
        !self.draft()
    }

    fn serialize_as_bool<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bool(self.draft())
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Extra {
    pub id: i64,
    pub language: String,
    pub author_name: String,
}

#[derive(Debug, Serialize)]
pub struct Post {
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub slug: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    // Sqlite uses UTC for all times by default:
    // <https://sqlite.org/lang_datefunc.html> section 2
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated: Option<DateTime<Utc>>,
    #[serde(
        skip_serializing_if = "Status::published",
        serialize_with = "Status::serialize_as_bool",
        rename = "draft"
    )]
    pub status: Status,

    pub extra: Extra,
    pub taxonomies: Taxonomies,

    #[serde(skip)]
    pub content: String,
}

#[derive(Debug, Default, Serialize)]
pub struct Taxonomies {
    tags: Vec<String>,
}

impl Post {
    pub fn query(conn: &Connection) -> Result<Vec<Post>, rusqlite::Error> {
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
        let mut out: Result<Vec<Post>, rusqlite::Error> = stmt
            .query_map(params![], |row| {
                Ok(Post {
                    // ID: 0
                    title: row.get(1)?,
                    content: row.get(2)?,
                    description: row.get(3)?,
                    date: row.get(4)?,
                    updated: row.get(5)?,
                    status: row.get(6)?,
                    slug: row.get(7)?,
                    extra: Extra {
                        id: row.get(0)?,
                        language: row.get(8)?,
                        author_name: row.get(9)?,
                    },
                    taxonomies: Taxonomies::default(),
                })
            })?
            .collect();

        if let Ok(posts) = &mut out {
            for post in posts.iter_mut() {
                post.update_tags(conn)?;
                post.content = relative_internal_links(&post.content);
            }
        }

        out
    }

    fn update_tags(&mut self, conn: &Connection) -> Result<(), rusqlite::Error> {
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
        self.taxonomies.tags = stmt
            .query_map(params![self.extra.id], |row| Ok(row.get::<_, String>(0)?))?
            .collect::<Result<Vec<String>, rusqlite::Error>>()?;
        Ok(())
    }

    pub fn render_to<W: Write>(&self, writer: &mut W) -> Result<(), crate::Error> {
        writeln!(writer, "+++")?;
        writeln!(writer, "{}", toml::to_string(self)?)?;
        writeln!(writer, "+++")?;
        writeln!(writer, "")?;
        writeln!(writer, "{}", self.content)?;
        Ok(())
    }

    /// construct a safe slug for this post
    ///
    /// - if a slug has already been set, use that
    /// - otherwise, construct one from the title
    /// - unless the title is empty, in which case use a uuidv4
    pub fn slug(&self) -> String {
        if self.slug.is_empty() {
            if self.title.is_empty() {
                uuid::Uuid::new_v4().to_string()
            } else {
                slugify!(&self.title, max_length = 150)
            }
        } else {
            self.slug.clone()
        }
    }

    /// return the relative path to which this post should be rendered
    pub fn relative_path(&self) -> PathBuf {
        let base = match self.date {
            Some(date) => PathBuf::new()
                .join(date.format("%Y").to_string())
                .join(date.format("%m").to_string())
                .join(date.format("%d").to_string()),
            None => PathBuf::from("undated"),
        };
        let name = PathBuf::from(self.slug()).with_extension("md");
        base.join(name)
    }
}

impl fmt::Display for Post {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut rendered = Vec::new();
        self.render_to(&mut rendered).map_err(|_| std::fmt::Error)?;
        // this is safe because we just populated the render with only valid utf-8
        write!(f, "{}", unsafe { String::from_utf8_unchecked(rendered) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_render() {
        let post = Post {
            title: "Fancy Example Post".into(),
            content: "I'm so fancy, I have paragraphs.\n\nSee!?".into(),
            description: String::new(),
            date: None,
            updated: None,
            status: Status::Draft,
            slug: "fancy-example-post".into(),
            extra: Extra {
                id: 123,
                language: "en_EN".into(),
                author_name: "me".into(),
            },
            taxonomies: Taxonomies {
                tags: vec!["tag1".into(), "another".into()],
            },
        };

        println!("{}", post.to_string());
        println!("=== next post ===");

        let post = Post {
            date: Some(Utc::now()),
            status: Status::Published,
            content: post.content + "\n\nAll Done",
            ..post
        };

        println!("{}", post.to_string());
    }

    mod replace_links {
        use super::super::*;

        fn replace_links(example: &str, expect: &str) {
            assert_eq!(relative_internal_links(example), expect);
        }

        #[test]
        fn test_should_replace_link() {
            replace_links("![](/content/images/2020/01/asdf.jpg)", "![](../asdf.jpg)");
        }


        #[test]
        fn test_should_skip_external_link() {
            let external ="![](https://photobucket.com/content/images/2020/01/asdf.jpg)";
            replace_links(external, external);
        }

        #[test]
        fn test_leaves_extra_markup() {
            replace_links("![very important pictures](/content/images/1234/56/fds.png)", "![very important pictures](../fds.png)");
        }

        #[test]
        fn test_big() {
            let gallery = "
            Hello, welcome to my gallery. I've included several pictures.

            ![](/content/images/2020/01/asdf.jpg)
            ![](https://photobucket.com/content/images/2020/01/asdf.jpg)
            ![very important pictures](/content/images/1234/56/fds.png)

            As you can see, they are phenomenal.
            ";

            let expect = "
            Hello, welcome to my gallery. I've included several pictures.

            ![](../asdf.jpg)
            ![](https://photobucket.com/content/images/2020/01/asdf.jpg)
            ![very important pictures](../fds.png)

            As you can see, they are phenomenal.
            ";

            replace_links(gallery, expect);
        }
    }
}
