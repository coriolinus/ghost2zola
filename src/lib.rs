pub mod data_model;

mod archive;
mod extract;
pub use archive::{find_ghost_db, find_ghost_db_in, find_ghost_dbs, try_archive};
pub use extract::extract_archive;

#[derive(Debug, thiserror::Error)]
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
    #[error("reading ghost database")]
    Sql(#[from] rusqlite::Error),
    #[error("generating frontmatter toml")]
    Frontmatter(#[from] toml::ser::Error),
}

pub(crate) fn log_progress(idx: usize, verb: &str) {
    if idx > 0 {
        if idx & 0x7fff == 0 {
            log::info!("{} {} archive entries", verb, idx);
        } else if idx & 0x1fff == 0 {
            log::trace!("{} {} archive entries", verb, idx);
        }
    }
}
