use clap::Parser;
use regex::Regex;

use std::{io, fs, path::PathBuf, error::Error};


#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    path: Option<PathBuf>,

    #[arg(short, long, value_name = "THREADID")]
    thread_id: Option<String>,
}


fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    let thread_re = if args.thread_id.is_some() {
        Regex::new(&args.thread_id.unwrap()).expect("Regex failed")
    } else {
        Regex::new(&".").expect("Regex failed")
    };

    let input = args.path;
    let mut reader: Box<dyn io::Read> = match input {
        None => Box::new(io::stdin()),
        Some(filename)   => Box::new(fs::File::open(filename).unwrap())
    };

    let mut buffer = String::new();
    reader.read_to_string(&mut buffer)?;
    let results: Vec<(&str, &str)> = buffer.lines()
        .filter_map(|line| {
            match thread_re.captures(line) {
                Some(capture) => Some((capture.get(0).unwrap().as_str(), line)),
                _ => None
            }
        })
        .collect();
    for r in results {
        println!("{} -> {}", r.0, r.1);
    }
    Ok(())
}
