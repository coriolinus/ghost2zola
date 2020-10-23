use ghost2zola::FileType;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Opt {
    /// Paths to check
    #[structopt(parse(from_os_str))]
    paths: Vec<PathBuf>,
}

fn main() {
    pretty_env_logger::init();

    let opt = Opt::from_args();
    let longest_path = opt
        .paths
        .iter()
        .map(|path| path.display().to_string().chars().count())
        .max();
    for path in &opt.paths {
        // we know that longest_path must not be None, so unwrap at will
        println!(
            "{path:>width$}: {type:30} {detected:?}",
            path=path.display(),
            width=longest_path.unwrap(),
            type=tree_magic::from_filepath(path),
            detected=FileType::try_from_path(path),
        );
    }
}
