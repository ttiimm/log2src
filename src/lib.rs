use regex::Regex;
use std::fmt;
use tree_sitter::{Parser, Query, QueryCursor};


#[derive(Debug)]
pub struct LogRef<'a> {
    id: &'a str,
    line_no: usize,
    pub text: &'a str
}

impl fmt::Display for LogRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] `{}`", self.id, self.text)
    }
}

#[derive(Debug)]
pub struct SourceRef<'a> {
    pub line_no: usize,
    col: usize,
    text: &'a str,
    matcher: Regex
}

impl fmt::Display for SourceRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[Line: {}, Col: {}] source `{}`", self.line_no, self.col, self.text)
    }
}

pub fn link_to_source<'a>(log_line: &LogRef, src_logs: &'a Vec<SourceRef>) -> Option<&'a SourceRef<'a>> {
    src_logs.iter()
            .find(|&e| e.matcher.is_match(log_line.text))
}


pub fn filter_log(buffer: &String, thread_re: Regex) -> Vec<LogRef> {
    let results = buffer.lines().enumerate()
        .filter_map(|(line_no, line)| {
            match thread_re.captures(line) {
                Some(capture) => {
                    let id = capture.get(0).unwrap().as_str();
                    let text = line;
                    Some(LogRef { id, line_no, text })
                },
                _ => None
            }
        })
        .collect();
    results
}

pub fn extract(source: &str) -> Vec<SourceRef> {
    let mut parser = Parser::new();
    parser.set_language(tree_sitter_rust::language()).expect("Error loading Rust grammar");

    let tree = parser.parse(&source, None).unwrap();
    let root_node = tree.root_node();

    let debug_macros = r#"(macro_invocation
                                 macro: (identifier) @macro-name
                                   (token_tree (string_literal) @log)
                                   (#eq? @macro-name "debug"))"#;
    let query = Query::new(tree_sitter_rust::language(), debug_macros)
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
                let unquoted = &source[range.start_byte + 1..range.end_byte - 1];
                let matcher = Regex::new(unquoted).unwrap();
                let result = SourceRef { line_no: line, col, text, matcher };
                matched.push(result);
        }
    };
    matched
}
