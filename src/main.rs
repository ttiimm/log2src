use clap::Parser as ClapParser;
use log2src::{
    build_graph, extract_logging, extract_variables, filter_log, find_possible_paths,
    link_to_source, Filter, LogMapping, SourceRef,
};
use serde_json;
use std::{collections::HashMap, error::Error, fs, io, path::PathBuf};

#[derive(ClapParser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(long, value_name = "SOURCE")]
    source: String,

    #[arg(short, long, value_name = "LOG")]
    log: Option<PathBuf>,

    #[arg(short, long, value_name = "START")]
    start: Option<usize>,

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

    let source = fs::read_to_string(&args.source).expect("Can read the source file");
    let src_logs = extract_logging(&source);
    let call_graph = build_graph(&source);

    // maybe should move this into a lib
    let log_mappings = filtered
        .iter()
        .map(|log_ref| {
            let src_ref: Option<&SourceRef<'_>> = link_to_source(&log_ref, &src_logs);
            let variables = src_ref.map_or(HashMap::new(), |src_ref| {
                extract_variables(&log_ref, src_ref)
            });
            let stack = src_ref.map_or(Vec::new(), |src_ref| {
                find_possible_paths(src_ref, &call_graph)
            });
            LogMapping {
                log_ref,
                src_ref,
                variables,
                stack,
            }
        })
        .collect::<Vec<LogMapping>>();

    for mapping in log_mappings {
        let serialized = serde_json::to_string(&mapping).unwrap();
        println!("{}", serialized);
    }

    Ok(())
}
