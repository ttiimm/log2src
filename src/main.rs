use clap::Parser as ClapParser;
use logdbg::filter_source;
use regex::Regex;
use std::{io, fs, path::PathBuf, error::Error};


#[derive(ClapParser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long, value_name = "LOG")]
    log: Option<PathBuf>,

    #[arg(short, long, value_name = "THREADID")]
    thread_id: Option<String>,

    #[arg(short, long, value_name = "SOURCE")]
    source: String
}


fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    let thread_re = if args.thread_id.is_some() {
        Regex::new(&args.thread_id.unwrap()).expect("Regex failed")
    } else {
        Regex::new(&".").expect("Regex failed")
    };

    let input = args.log;
    let mut reader: Box<dyn io::Read> = match input {
        None => Box::new(io::stdin()),
        Some(filename)   => Box::new(fs::File::open(filename).unwrap())
    };

    let mut buffer = String::new();
    reader.read_to_string(&mut buffer)?;
    let filtered = logdbg::filter_log(&buffer, thread_re);
    print_result(filtered);

    let source = fs::read_to_string(&args.source)
        .expect("Can read the source file");
    let matched = filter_source(&source);
    print_result(matched);
    Ok(())
}

fn print_result(result: Vec<String>) {
    for r in result {
        println!("{}", r);
    }
}
