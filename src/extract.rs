use crate::{data_model::Post, find_ghost_db_in, log_progress, try_archive, Error};
use log;
use path_absolutize::Absolutize;
use rusqlite::Connection;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

struct PartialExtraction {
    database: NamedTempFile,
    images: Vec<PathBuf>,
}

impl PartialExtraction {
    fn new() -> Result<PartialExtraction, Error> {
        Ok(PartialExtraction {
            database: NamedTempFile::new()?,
            images: Vec::new(),
        })
    }
}

macro_rules! contextualize {
    ($e:expr) => {
        contextualize!($e; stringify!($e))
    };
    ($e:expr; $($c:expr),+) => {
        ($e).map_err(|e| {log::error!($($c),+); e})
    };
}

/// extract images and database from an archive
///
/// # Image Handling
///
/// Assuming that the ghost DB is located in `a/b/c/data/ghost.db`, in a standard configuration,
/// the images will be located in `a/b/c/images/yyyy/mm/*`. They will be extracted into
/// `extract_path/yyyy/mm/*`.
///
/// # Database Handling
///
/// To avoid memory issues with large databases, the database is extracted into a temporary file.
/// This file will be automatically removed by the OS when it is closed.
fn extract_images_and_db<AP>(
    archive_path: AP,
    prefix: Option<PathBuf>,
    extract_path: &Path,
) -> Result<PartialExtraction, Error>
where
    AP: AsRef<Path>,
{
    let archive_path = archive_path.as_ref();
    let extract_path = contextualize!(extract_path.canonicalize())?;
    let db_path = contextualize!(find_ghost_db_in(archive_path, prefix))?;
    let images_base = db_path
        .parent()
        .and_then(|parent| parent.parent())
        .map(|grandparent| grandparent.join("images"));

    log::info!("processing archive");
    let mut archive = contextualize!(try_archive(archive_path))?;
    let mut out = contextualize!(PartialExtraction::new())?;
    for (idx, entry) in contextualize!(archive.entries())?.enumerate() {
        log_progress(idx, "processed");

        let mut entry = contextualize!(entry)?;
        let path = contextualize!(entry.path())?;
        if path == db_path {
            // handle the database itself
            contextualize!(std::io::copy(&mut entry, &mut out.database))?;
            log::info!("extracted database at entry {}", idx);
        } else if entry.header().entry_type() == tar::EntryType::Directory
            || path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                == Some(String::from("md"))
        {
            // don't waste time on directories; we can unpack them on demand later
            // likewise, it's more trouble than it's worth to copy over markdown files
            continue;
        } else if let Some(images_base) = &images_base {
            if path.starts_with(images_base) {
                // handle an image
                let subpath = contextualize!(path.strip_prefix(images_base))?;
                let extract_to =
                    contextualize!((&extract_path).join(subpath).absolutize())?.to_path_buf();
                if !extract_to.starts_with(&extract_path) {
                    log::warn!(
                        "malicious file in tar attempted to extract past extraction root: {}",
                        subpath.display(),
                    );
                    continue;
                }
                if let Some(parent) = extract_to.parent() {
                    contextualize!(std::fs::create_dir_all(parent))?;
                }
                log::trace!("extracting image: {}", extract_to.display());
                contextualize!(entry.unpack(&extract_to))?;
                out.images.push(extract_to);
            }
        }
    }
    log::info!("extracted {} images", out.images.len());

    Ok(out)
}

/// Extract an archive into a destination folder.
///
/// # Image Handling
///
/// Assuming that the ghost DB is located in `a/b/c/data/ghost.db`, in a standard configuration,
/// the images will be located in `a/b/c/images/yyyy/mm/*`. They will be extracted into
/// `extract_path/yyyy/mm/*`.
///
/// # Post Handling
///
/// Posts are extracted from the Ghost-format sqlite DB and converted into Zola-compatible format.
///
/// **WARN: if the post's original markdown has been lost, i.e. from a previous Ghost import, it will be skipped!**
/// In that circumstance, consider regenerating the markdown from the rendered post content within the database
/// with a different tool.
///
/// Each post will be extracted into `extract_path/yyyy/mm/dd/slug`.
///
/// ## Self-hosted images
///
/// Within each post's markdown, things which look like image links, i.e. things which match the regex
/// `\]\(/content/images/\d{4}/\d{2}/[^)]+\)`, will have the `/content/images` portion stripped out and
/// replaced with `/blog`, ending up as `](/blog/dddd/mm/$1)`. This should preserve the links.
///
/// ## Metadata
///
/// Zola expects post metadata to exist in TOML front matter prepended to each post. The following metadata
/// is extracted from the DB and rendered into the frontmatter:
///
/// | Ghost Sql Field | Zola Frontmatter Key | Notes |
/// | --- | --- | --- |
/// | `title` | `title` | |
/// | `meta_description` | `description` | not set if empty |
/// | `published_at` | `date` | not set if empty |
/// | `updated_at` | `updated` | not set if empty |
/// | `status` | `draft` | `"published"` => `false`; anything else => `true`; not set if false |
/// | `slug` | `slug` | |
/// | `language` | `extra.language` | |
/// | `users.name` | `extra.author_name` | `posts inner join users on posts.author_id = users.id` |
/// | `tags.name` | `taxonomies.tags` | `select tags.name from posts_tags inner join tags on posts_tags.tag_id = tags.id where posts_tags.post_id = %` |
pub fn extract_archive<AP, EP>(
    archive_path: AP,
    prefix: Option<PathBuf>,
    extract_path: EP,
) -> Result<usize, Error>
where
    AP: AsRef<Path>,
    EP: AsRef<Path>,
{
    let extract_path = extract_path.as_ref();
    extract_images_and_db(archive_path, prefix, extract_path)?.extract_database(extract_path)
}

impl PartialExtraction {
    fn extract_database(self, extract_path: &Path) -> Result<usize, Error> {
        let conn = Connection::open_with_flags(
            self.database.path(),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let posts = Post::query(&conn)?;
        for post in posts.iter() {
            let relative_path = post.relative_path();
            let path = extract_path.join(&relative_path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(path)?;
            let mut writer = std::io::BufWriter::new(file);
            post.render_to(&mut writer)?;
            log::trace!("generated {}", relative_path.display());
        }
        log::info!("extracted {} posts", posts.len());

        // now ensure that appropriate indices exist
        let n_indices = ensure_indices(extract_path)?;
        log::info!("added {} indices", n_indices);

        Ok(posts.len())
    }
}

const ROOT_INDEX_DATA: &[u8] = include_bytes!("../templates/root._index.md");
const BRANCH_INDEX_DATA: &[u8] = include_bytes!("../templates/branch._index.md");

fn ensure_indices(extract_path: &Path) -> Result<u32, Error> {
    let mut n = 0;

    let index = extract_path.join("_index.md");
    if !index.exists() {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(index)?;
        file.write_all(ROOT_INDEX_DATA)?;
        n += 1;
    }

    for subdir in extract_path.read_dir()?.filter(|maybe_dir_entry| {
        maybe_dir_entry
            .as_ref()
            .map(|dir_entry| {
                dir_entry
                    .file_type()
                    .map(|file_type| file_type.is_dir())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
    }) {
        let subdir = match subdir {
            Ok(subdir) => subdir,
            Err(e) => {
                log::error!(
                    "failed to read subdirectory of {}: {:#?}",
                    extract_path.display(),
                    e
                );
                continue;
            }
        };

        n += ensure_indices_recursive(&subdir.path())?;
    }

    /// Recursive mode on!
    fn ensure_indices_recursive(path: &Path) -> Result<u32, Error> {
        let mut n = 0;

        let index = path.join("_index.md");
        if !index.exists() {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .open(index)?;
            file.write_all(BRANCH_INDEX_DATA)?;
            n += 1;
        }

        for subdir in path.read_dir()?.filter(|maybe_dir_entry| {
            maybe_dir_entry
                .as_ref()
                .map(|dir_entry| {
                    dir_entry
                        .file_type()
                        .map(|file_type| file_type.is_dir())
                        .unwrap_or_default()
                })
                .unwrap_or_default()
        }) {
            let subdir = match subdir {
                Ok(subdir) => subdir,
                Err(e) => {
                    log::error!(
                        "failed to read subdirectory of {}: {:#?}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };

            n += ensure_indices_recursive(&subdir.path())?;
        }

        Ok(n)
    }

    Ok(n)
}
