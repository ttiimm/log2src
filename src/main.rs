use clap::Parser as ClapParser;
use regex::Regex;
use std::{io, fs, path::PathBuf, error::Error};
use tree_sitter::{Parser, Query, QueryCursor};


#[derive(ClapParser)]
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
    let filtered = logdbg::filter_log(&buffer, thread_re);

    for r in filtered {
        println!("{} -> {}", r.0, r.1);
    }

    let mut parser = Parser::new();
    parser.set_language(tree_sitter_rust::language()).expect("Error loading Rust grammar");

    let source = fs::read_to_string("examples/basic.rs")
        .expect("Expecting a readable file");
    let tree = parser.parse(&source, None).unwrap();
    let root_node = tree.root_node();

    let log_strings = r#"(macro_invocation
                                 macro: (identifier) @macro-name
                                   (token_tree (string_literal) @log)
                                 (#eq? @macro-name "debug"))"#;
    let query = Query::new(tree_sitter_rust::language(), log_strings)
        .unwrap();
    let mut query_cursor = QueryCursor::new();
    let matches = query_cursor.matches(
        &query, root_node, source.as_bytes()
    );

    for m in matches {
        for capture in m
            .captures
            .iter()
            .filter(|c| c.node.kind() == "string_literal") {
                let range = capture.node.range();
                let text = &source[range.start_byte..range.end_byte];
                let line = range.start_point.row + 1;
                let col = range.start_point.column;
                println!(
                    "[Line: {}, Col: {}] Offending source code: `{}`",
                    line, col, text
                );
        }
    }

    Ok(())
}
