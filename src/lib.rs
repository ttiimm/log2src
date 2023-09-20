use regex::Regex;
use tree_sitter::{Parser, Query, QueryCursor};


pub fn filter_log(buffer: &String, thread_re: Regex) -> Vec<String> {
    let results: Vec<String> = buffer.lines()
        .filter_map(|line| {
            match thread_re.captures(line) {
                Some(capture) => {
                    let result = format!("{} -> {}", capture.get(0).unwrap().as_str(), line);
                    Some(result)
                },
                _ => None
            }
        })
        .collect();
    results
}

pub fn filter_source(source: &str) -> Vec<String> {
    let mut parser = Parser::new();
    parser.set_language(tree_sitter_rust::language()).expect("Error loading Rust grammar");

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

    let mut matched = Vec::new();
    for m in matches {
        for capture in m
            .captures
            .iter()
            .filter(|c| c.node.kind() == "string_literal") {
                let range = capture.node.range();
                let text = &source[range.start_byte..range.end_byte];
                let line = range.start_point.row + 1;
                let col = range.start_point.column;
                let result = format!("[Line: {}, Col: {}] source `{}`", line, col, text);
                matched.push(result);
        }
    };
    matched
}