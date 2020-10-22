use ghost2zola::{find_ghost_db, try_archive};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Path to check
    #[structopt(parse(from_os_str))]
    path: PathBuf,
}

fn main() -> Result<(), anyhow::Error> {
    let opt = Opt::from_args();
    let mut archive = try_archive(&opt.path)?;
    for db_path in find_ghost_db(&mut archive)? {
        println!("{}", db_path.display());
    }
    Ok(())
}
