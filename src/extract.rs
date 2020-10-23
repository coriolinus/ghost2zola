use crate::{find_ghost_db_in, try_archive, Error};
use log;
use std::path::{Path, PathBuf};
use tempfile::tempfile;

struct PartialExtraction {
    database: std::fs::File,
    images: Vec<PathBuf>,
}

impl PartialExtraction {
    fn new() -> Result<PartialExtraction, Error> {
        Ok(PartialExtraction {
            database: tempfile()?,
            images: Vec::new(),
        })
    }
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
    extract_path: PathBuf,
) -> Result<PartialExtraction, Error>
where
    AP: AsRef<Path>,
{
    let archive_path = archive_path.as_ref();
    let db_path = find_ghost_db_in(archive_path, prefix)?;
    let images_base = db_path
        .parent()
        .and_then(|parent| parent.parent())
        .map(|grandparent| grandparent.join("images"));

    log::info!("processing archive");
    let mut archive = try_archive(archive_path)?;
    let mut out = PartialExtraction::new()?;
    for (idx, entry) in archive.entries()?.enumerate() {
        if idx > 0 {
            if idx & 0x3fff == 0 {
                log::info!("processed {} archive entries", idx);
            } else if idx & 0xfff == 0 {
                log::trace!("processed {} archive entries", idx);
            }
        }

        let mut entry = entry?;
        let path = entry.path()?;
        if path == db_path {
            // handle the database itself
            std::io::copy(&mut entry, &mut out.database)?;
            log::info!("extracted database at entry {}", idx);
        } else if let Some(images_base) = &images_base {
            if path.starts_with(images_base) {
                // handle an image
                let subpath = path.strip_prefix(images_base)?;
                let extract_to = extract_path.join(subpath).canonicalize()?;
                if !extract_to.starts_with(images_base) {
                    log::warn!(
                        "malicious file in tar attempted to extract past extraction root: {}",
                        subpath.display(),
                    );
                    continue;
                }
                if let Some(parent) = extract_to.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                log::trace!("extracting image: {}", subpath.display());
                entry.unpack(&extract_to)?;
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
/// `\]\(/content/images/\d{4}/\d{2}/[^)]+\)`, will have their absolute paths stripped out and replaced
/// with relative paths, ending up as `](../$1)`. This should preserve the links.
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
pub fn extract_archive<AP>(
    archive_path: AP,
    prefix: Option<PathBuf>,
    extract_path: PathBuf,
) -> Result<usize, Error>
where
    AP: AsRef<Path>,
{
    extract_images_and_db(archive_path, prefix, extract_path)?.extract_database()
}

impl PartialExtraction {
    fn extract_database(self) -> Result<usize, Error> {
        unimplemented!()
    }
}
