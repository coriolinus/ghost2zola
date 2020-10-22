use std::{io::Read, path::PathBuf};

use structopt::StructOpt;

use ghost2zola::ghost::Top;

#[derive(Debug, StructOpt)]
struct Opt {
    /// JSON input file, stdin if not present
    #[structopt(parse(from_os_str))]
    input: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let reader: Box<dyn Read> = if let Some(path) = opt.input {
        Box::new(std::fs::File::open(path)?)
    } else {
        Box::new(std::io::stdin())
    };
    let reader = std::io::BufReader::new(reader);
    match serde_json::from_reader::<_, Top>(reader) {
        Ok(_) => {
            println!("parsed ok!");
        }
        Err(e) => eprintln!("{:#?}", e),
    }

    Ok(())
}
