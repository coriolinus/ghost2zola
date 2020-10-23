use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
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
