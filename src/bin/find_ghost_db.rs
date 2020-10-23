use ghost2zola::{find_ghost_db, find_ghost_dbs, try_archive};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Path to check
    #[structopt(parse(from_os_str))]
    path: PathBuf,

    /// Prefix to search for db within
    #[structopt(parse(from_os_str), long)]
    prefix: Option<PathBuf>,

    /// Find all possible DB paths instead of searching for a single one
    #[structopt(long)]
    all: bool,
}

fn main() -> Result<(), anyhow::Error> {
    let opt = Opt::from_args();
    let mut archive = try_archive(&opt.path)?;
    if opt.all {
        for db_path in find_ghost_dbs(&mut archive)? {
            println!("{}", db_path.display());
        }
    } else {
        let db_path = find_ghost_db(&mut archive, opt.prefix)?;
        println!("found db path: {}", db_path.display());
    }
    Ok(())
}
