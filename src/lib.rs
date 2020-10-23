use log;
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use tempfile::tempfile;
use thiserror::Error;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum FileType {
    Sqlite3,
    Tar,
    TarGz,
    TarBz2,
}

impl FileType {
    pub fn try_from_path(path: &Path) -> Option<Self> {
        match tree_magic::from_filepath(path).as_str() {
            "application/vnd.sqlite3" => Some(FileType::Sqlite3),
            "application/x-tar" => Some(FileType::Tar),
            "application/gzip" => Some(FileType::TarGz),
            "application/x-bzip" => Some(FileType::TarBz2),
            _ => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("IO")]
    Io(#[from] std::io::Error),
    #[error("input does not appear to be a (compressed) tar file")]
    NotTar,
    #[error("input does not contain a ghost.db within search area")]
    GhostDbNotFound,
    #[error("input contains more than one ghost.db within search area")]
    MultipleGhostDb,
    #[error("failed to strip an image prefix")]
    StripPrefix(#[from] std::path::StripPrefixError),
}

fn try_to_tar_reader(path: &Path) -> Result<Box<dyn Read>, Error> {
    let reader = std::fs::File::open(&path)?;
    let reader = std::io::BufReader::new(reader);
    match FileType::try_from_path(&path) {
        Some(FileType::Tar) => Ok(Box::new(reader)),
        Some(FileType::TarGz) => {
            let reader = libflate::gzip::Decoder::new(reader)?;
            Ok(Box::new(reader))
        }
        Some(FileType::TarBz2) => {
            let reader = bzip2::read::BzDecoder::new(reader);
            Ok(Box::new(reader))
        }
        _ => Err(Error::NotTar),
    }
}

pub fn try_archive(path: &Path) -> Result<tar::Archive<Box<dyn Read>>, Error> {
    let reader = try_to_tar_reader(path)?;
    Ok(tar::Archive::new(reader))
}

pub fn find_ghost_dbs<'a, R>(
    archive: &'a mut tar::Archive<R>,
) -> Result<impl 'a + Iterator<Item = PathBuf>, Error>
where
    R: 'a + Read,
{
    Ok(archive.entries()?.filter_map(|maybe_entry| {
        maybe_entry
            .ok()
            .map(|entry| {
                entry
                    .path()
                    .ok()
                    .filter(|path| path.file_name() == Some(OsStr::new("ghost.db")))
                    .map(|path| path.into_owned())
            })
            .flatten()
    }))
}

fn conditional_filter<'a>(
    iter: impl 'a + Iterator<Item = PathBuf>,
    prefix: Option<PathBuf>,
) -> Box<dyn 'a + Iterator<Item = PathBuf>> {
    match prefix {
        None => Box::new(iter),
        Some(prefix) => {
            let filter = move |path: &PathBuf| path.starts_with(prefix.as_path());
            Box::new(iter.filter(filter))
        }
    }
}

pub fn find_ghost_db<R>(
    archive: &mut tar::Archive<R>,
    prefix: Option<PathBuf>,
) -> Result<PathBuf, Error>
where
    R: Read,
{
    let db_iter = find_ghost_dbs(archive)?;
    let db_iter = conditional_filter(db_iter, prefix);
    let mut dbs: Vec<_> = db_iter.take(2).collect();
    match dbs.len() {
        0 => Err(Error::GhostDbNotFound),
        2 => Err(Error::MultipleGhostDb),
        1 => Ok(std::mem::take(&mut dbs[0])),
        _ => unreachable!(),
    }
}

/// find the internal path to a ghost db in an archive
pub fn find_ghost_db_in<P: AsRef<Path>>(
    path: P,
    prefix: Option<PathBuf>,
) -> Result<PathBuf, Error> {
    log::info!("analyzing archive");
    let mut archive = try_archive(path.as_ref())?;
    find_ghost_db(&mut archive, prefix)
}

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
