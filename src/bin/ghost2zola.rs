use ghost2zola::{extract_archive};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Path to a possibly-compressed tar archiving a ghost blog
    #[structopt(parse(from_os_str))]
    archive_path: PathBuf,

    /// Path to the base directory into which the ghost blog should be expanded.
    ///
    /// Normally, this is the `content/blog` directory of your zola installation.
    #[structopt(parse(from_os_str))]
    extract_path: PathBuf,

    /// Relative prefix within the archive
    ///
    /// In cases where the archive contains only a single blog, this is not necessary.
    /// When the archive contains several blogs, this can be set to any distinct prefix
    /// winnowing the selection to a single selection.
    ///
    /// If you're not sure what prefixes might be available, consider using the `find_ghost_db` tool.
    #[structopt(parse(from_os_str), long)]
    prefix: Option<PathBuf>,
}

fn main() -> Result<(), anyhow::Error> {
    pretty_env_logger::init();
    let opt = Opt::from_args();

    extract_archive(opt.archive_path, opt.prefix, opt.extract_path)?;

    Ok(())
}
