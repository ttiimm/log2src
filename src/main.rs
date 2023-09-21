use clap::Parser as ClapParser;
use logdbg::{extract, filter_log, link};
use regex::Regex;
use std::{io, fs, path::PathBuf, error::Error};
mod ui;


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
        Regex::new(&args.thread_id.unwrap()).expect("Valid regex")
    } else {
        Regex::new(&"^").unwrap()
    };

    let input = args.log;
    let mut reader: Box<dyn io::Read> = match input {
        None => Box::new(io::stdin()),
        Some(filename)   => Box::new(fs::File::open(filename)
            .expect("Can open file"))
    };

    let mut buffer = String::new();
    reader.read_to_string(&mut buffer)?;
    let filtered = filter_log(&buffer, thread_re);

    let source = fs::read_to_string(&args.source)
        .expect("Can read the source file");
    let src_logs = extract(&source);

    for f in filtered {
        let result = link(&f, &src_logs);
        println!("{} -> {:?}", f, result);
    }

    ui::start(&source);

    Ok(())
}
