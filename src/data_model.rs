use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use regex::{Regex, RegexBuilder};
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
        RegexBuilder::new(r"\]\(/content/images/(\d{4}/\d{2}/[^)]+)\)")
            .case_insensitive(true)
            .build()
            .unwrap();
    static ref DATE_QUOTE_STRIP_RE: Regex =
        RegexBuilder::new(r#"^(date|updated) = "([- \w\d:\.]+)"$"#)
            .multi_line(true)
            .build()
            .unwrap();
    static ref PRE_REIFIED_FOOTNOTES: Regex = Regex::new(r"\[\^(\d+)\]").unwrap();
    static ref FOOTNOTE_FOOT: Regex = RegexBuilder::new(r"^\[\^n\]:")
        .multi_line(true)
        .build()
        .unwrap();
    static ref FOOTNOTE_TEXT: Regex = Regex::new(r"\[\^n\]").unwrap();
}

/// replace internal hardlinks with relative links to the parent
pub(crate) fn relative_internal_links(text: &str) -> String {
    INTERNAL_LINK_RE
        .replace_all(text, "](/blog/$1)")
        .into_owned()
}

/// strip quotation marks from toml fields named `date` or `updated`
pub(crate) fn strip_datetime_quotes(text: &str) -> String {
    DATE_QUOTE_STRIP_RE
        .replace_all(text, "$1 = $2")
        .into_owned()
}

/// Replace all detected abstract footnotes with numbered ones.
///
/// Ghost has a somewhat more advanced notion of footnotes than Zola does: you can use `[^n]` to insert
/// a footnote anywhere, and `[^n]:`, and both matching and linking happen automatically.
///
/// Zola is a little less smart about this: you only get one-way links, and all `[^n]` get replaced with `[^1]`.
/// This isn't the most useful thing. Therefore, we have to replace all `[^n]` with actual numbers, not clobbering
/// any other footnotes already injected.
///
/// This implementation numbers weirdly if someone has already inserted any hard numbered footnotes interspersed
/// with the generated ones, but that's their problem for doing it wrong.
pub(crate) fn reify_footnotes(s: &str) -> String {
    // first, go through the existing numbered footnotes and find the highest
    let highest_existing: u32 = PRE_REIFIED_FOOTNOTES
        .captures_iter(s)
        .map(|capture| {
            capture
                .get(1)
                .map(|mtch| mtch.as_str().parse().unwrap_or_default())
                .unwrap_or_default()
        })
        .max()
        .unwrap_or_default();

    let mut text = s.to_string();

    // sequentially replace all footer footnote anchors with incrementing numbers
    let mut idx = highest_existing;
    loop {
        idx += 1;
        let mut new = FOOTNOTE_FOOT
            .replace(&text, format!("[^{}]:", idx).as_str())
            .to_string();
        std::mem::swap(&mut text, &mut new);
        if text == new {
            break;
        }
    }

    // now do it again for the text footnote anchors
    let mut idx = highest_existing;
    loop {
        idx += 1;
        let mut new = FOOTNOTE_TEXT
            .replace(&text, format!("[^{}]", idx).as_str())
            .to_string();
        std::mem::swap(&mut text, &mut new);
        if text == new {
            break;
        }
    }

    text
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
                    // content and description are possibly null; we want to map those to empty strings
                    content: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    description: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
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

    fn render_toml(&self) -> Result<String, crate::Error> {
        // this is necessary because the TOML library doesn't handle TOML datetimes, emitting strings instead
        // we have to work around that
        Ok(strip_datetime_quotes(&toml::to_string(self)?))
    }

    pub fn render_to<W: Write>(&self, writer: &mut W) -> Result<(), crate::Error> {
        writeln!(writer, "+++")?;
        writeln!(writer, "{}", self.render_toml()?)?;
        writeln!(writer, "+++")?;
        writeln!(writer, "")?;
        writeln!(writer, "{}", reify_footnotes(&self.content))?;
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
            replace_links(
                "![](/content/images/2020/01/asdf.jpg)",
                "![](/blog/2020/01/asdf.jpg)",
            );
        }

        #[test]
        fn test_should_skip_external_link() {
            let external = "![](https://photobucket.com/content/images/2020/01/asdf.jpg)";
            replace_links(external, external);
        }

        #[test]
        fn test_leaves_extra_markup() {
            replace_links(
                "![very important pictures](/content/images/1234/56/fds.png)",
                "![very important pictures](/blog/1234/56/fds.png)",
            );
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

            ![](/blog/2020/01/asdf.jpg)
            ![](https://photobucket.com/content/images/2020/01/asdf.jpg)
            ![very important pictures](/blog/1234/56/fds.png)

            As you can see, they are phenomenal.
            ";

            replace_links(gallery, expect);
        }
    }

    #[test]
    fn strip_quotes_from_date() {
        let input = r#"
title = "Fancy Example Post"
slug = "fancy-example-post"
date = "2020-10-23T20:13:54.069963100Z"

[extra]
id = 123
language = "en_EN"
author_name = "me"

[taxonomies]
tags = ["tag1", "another"]
"#;
        let expect = r#"
title = "Fancy Example Post"
slug = "fancy-example-post"
date = 2020-10-23T20:13:54.069963100Z

[extra]
id = 123
language = "en_EN"
author_name = "me"

[taxonomies]
tags = ["tag1", "another"]
"#;
        assert_eq!(strip_datetime_quotes(input), expect);
    }

    #[test]
    fn strip_quotes_from_update() {
        let input = r#"
title = "Fancy Example Post"
slug = "fancy-example-post"
date = "2020-10-23T20:13:54.069963100Z"
updated = "2020-10-23T20:13:54.069963101Z"

[extra]
id = 123
language = "en_EN"
author_name = "me"

[taxonomies]
tags = ["tag1", "another"]
"#;
        let expect = r#"
title = "Fancy Example Post"
slug = "fancy-example-post"
date = 2020-10-23T20:13:54.069963100Z
updated = 2020-10-23T20:13:54.069963101Z

[extra]
id = 123
language = "en_EN"
author_name = "me"

[taxonomies]
tags = ["tag1", "another"]
"#;
        assert_eq!(strip_datetime_quotes(input), expect);
    }

    #[test]
    fn test_reify_footnotes_basic() {
        let input = "
Lorem ipsum dolor sit amet, consectetur adipiscing elit[^n]. Aenean sollicitudin velit tellus, in dignissim tellus venenatis pulvinar.
Suspendisse rhoncus nisi purus, ut convallis lectus placerat[^n] eget. Integer imperdiet eu nibh vitae tempor. Etiam at tristique enim.
Mauris malesuada nibh sit amet ligula mollis, eu interdum ipsum[^n] faucibus. Ut rutrum sapien ligula, at dapibus dui vestibulum id. Donec bibendum
felis finibus rhoncus gravida. Nulla facilisi. Aenean lacinia consectetur condimentum. Curabitur venenatis erat ex[^n], non auctor lorem sodales sit amet.
Nam id ultrices mauris, et malesuada nisl. Aenean diam risus[^n], lobortis eget accumsan vitae, accumsan non eros. Quisque ac tincidunt quam,
gravida tempor magna. Praesent pretium[^n] bibendum ante, et varius orci fermentum ac. Proin a tortor a nunc placerat pellentesque id ac ligula.

[^n]: Duis commodo venenatis efficitur.
[^n]: Aliquam semper convallis augue, non faucibus mauris commodo non. Pellentesque eget velit sed nunc lacinia tempus ac non erat.
[^n]: Donec vel augue in arcu porttitor interdum.
[^n]: Nunc consequat, risus ut scelerisque ornare, erat ligula ullamcorper turpis, sit amet eleifend justo lorem id ipsum.
[^n]: Interdum et malesuada fames ac ante ipsum primis in faucibus.
[^n]: Nullam eget nunc eget ante auctor finibus sit amet vitae tortor.
        ".trim();

        let expect = "
Lorem ipsum dolor sit amet, consectetur adipiscing elit[^1]. Aenean sollicitudin velit tellus, in dignissim tellus venenatis pulvinar.
Suspendisse rhoncus nisi purus, ut convallis lectus placerat[^2] eget. Integer imperdiet eu nibh vitae tempor. Etiam at tristique enim.
Mauris malesuada nibh sit amet ligula mollis, eu interdum ipsum[^3] faucibus. Ut rutrum sapien ligula, at dapibus dui vestibulum id. Donec bibendum
felis finibus rhoncus gravida. Nulla facilisi. Aenean lacinia consectetur condimentum. Curabitur venenatis erat ex[^4], non auctor lorem sodales sit amet.
Nam id ultrices mauris, et malesuada nisl. Aenean diam risus[^5], lobortis eget accumsan vitae, accumsan non eros. Quisque ac tincidunt quam,
gravida tempor magna. Praesent pretium[^6] bibendum ante, et varius orci fermentum ac. Proin a tortor a nunc placerat pellentesque id ac ligula.

[^1]: Duis commodo venenatis efficitur.
[^2]: Aliquam semper convallis augue, non faucibus mauris commodo non. Pellentesque eget velit sed nunc lacinia tempus ac non erat.
[^3]: Donec vel augue in arcu porttitor interdum.
[^4]: Nunc consequat, risus ut scelerisque ornare, erat ligula ullamcorper turpis, sit amet eleifend justo lorem id ipsum.
[^5]: Interdum et malesuada fames ac ante ipsum primis in faucibus.
[^6]: Nullam eget nunc eget ante auctor finibus sit amet vitae tortor.
        ".trim();

        assert_eq!(reify_footnotes(input), expect);
    }
}
