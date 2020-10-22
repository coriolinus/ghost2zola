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
    #[error("Input does not appear to be a (compressed) tar file")]
    NotTar,
}

fn try_to_tar_reader(path: &Path) -> Result<Box<dyn Read>, Error> {
    let reader = std::fs::File::open(&path)?;
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

pub fn find_ghost_db<'a, R>(
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
