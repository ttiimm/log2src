use clap::Parser as ClapParser;
use log2src::{do_mappings, extract_logging, filter_log, find_code, CallGraph, Filter};
use serde_json::{self};
use std::{error::Error, fs, io, path::PathBuf};

/// The log2src command maps log statements back to the source code that emitted them.
#[derive(ClapParser)]
#[command(author, version, about, long_about)]
struct Cli {
    /// A source directory (or soon directoires) to map logs onto
    #[arg(short = 'd', long, value_name = "SOURCES")]
    sources: String,

    /// A log file to use, if not from stdin
    #[arg(short, long, value_name = "LOG")]
    log: Option<PathBuf>,

    /// The line in the log to use (0 based)
    #[arg(short, long, value_name = "START")]
    start: Option<usize>,

    /// The last line of the log to use (0 based)
    #[arg(short, long, value_name = "END")]
    end: Option<usize>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();
    let input = args.log;
    let mut reader: Box<dyn io::Read> = match input {
        None => Box::new(io::stdin()),
        Some(filename) => Box::new(fs::File::open(filename).expect("Can open file")),
    };

    let mut buffer = String::new();
    reader.read_to_string(&mut buffer)?;
    let filter = Filter {
        start: args.start.unwrap_or(0),
        end: args.end.unwrap_or(usize::MAX),
    };
    let filtered = filter_log(&buffer, filter);

    let mut sources = find_code(&args.sources);
    let src_logs = extract_logging(&mut sources);
    let call_graph = CallGraph::new(&mut sources);
    let log_mappings = do_mappings(&filtered, &src_logs, &call_graph);

    for mapping in log_mappings {
        let serialized = serde_json::to_string(&mapping).unwrap();
        println!("{}", serialized);
    }

    Ok(())
}
