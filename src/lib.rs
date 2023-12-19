use regex::Regex;
use std::fmt;
use std::collections::HashMap;
use serde::Serialize;
use tree_sitter::{Node, Parser, Query, QueryCursor};


#[derive(Serialize)]
pub struct LogMapping<'a> {
    #[serde(skip_serializing)]
    pub log_ref: &'a LogRef<'a>,
    #[serde(rename(serialize = "srcRef"))]
    pub src_ref: Option<&'a SourceRef<'a>>,
    pub variables: HashMap<&'a str, &'a str>,
}


#[derive(Debug)]
pub struct LogRef<'a> {
    id: &'a str,
    _line_no: usize,
    pub text: &'a str,
}

impl fmt::Display for LogRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] `{}`", self.id, self.text)
    }
}


#[derive(Debug, Serialize)]
pub struct SourceRef<'a> {
    #[serde(rename(serialize = "lineNumber"))]
    pub line_no: usize,
    column: usize,
    name: &'a str,
    text: &'a str,
    #[serde(skip_serializing)]
    matcher: Regex,
    vars: Vec<&'a str>,
}

impl fmt::Display for SourceRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[Line: {}, Col: {}] source `{}` name `{}` vars={:?}",
            self.line_no, self.column, self.text, self.name, self.vars
        )
    }
}

pub fn link_to_source<'a>(
    log_line: &LogRef,
    src_logs: &'a Vec<SourceRef>,
) -> Option<&'a SourceRef<'a>> {
    src_logs.iter().find(|&source_ref| {
        if let Some(_) = source_ref.matcher.captures(log_line.text) {
            return true;
        }
        false
    })
}

pub fn extract_variables<'a>(
    log_line: &'a LogRef,
    src_ref: &'a SourceRef
) -> HashMap<&'a str, &'a str> {
    let mut variables = HashMap::new();
    if src_ref.vars.len() > 0 {
        if let Some(captures) = src_ref.matcher.captures(log_line.text) {
            for i in 0..captures.len() - 1 {
                variables.insert(src_ref.vars[i], captures.get(i + 1).unwrap().as_str());
            }
        }
    }

    variables
}

pub fn filter_log(buffer: &String, thread_re: Regex, start: usize, end: usize) -> Vec<LogRef> {
    let results = buffer
        .lines()
        .enumerate()
        .filter(|(line_no, _line)|  start <= *line_no && *line_no < end)
        .filter_map(|(_line_no, line)| match thread_re.captures(line) {
            Some(capture) => {
                let id = capture.get(0).unwrap().as_str();
                let text = line;
                Some(LogRef { id, _line_no, text })
            }
            _ => None,
        })
        .collect();
    results
}

pub fn extract_source(source: &str) -> Vec<SourceRef> {
    let mut parser = Parser::new();
    parser.set_language(tree_sitter_rust::language())
          .expect("Error loading Rust grammar");

    let tree = parser.parse(&source, None).unwrap();
    let root_node = tree.root_node();
    // println!("{:?}", root_node.to_sexp());

    let debug_macros = r#"
        (macro_invocation macro: (identifier) @macro-name
            (token_tree
                (string_literal) @log (identifier)* @arguments
            ) (#eq? @macro-name "debug")
        )
    "#;
    let query = Query::new(tree_sitter_rust::language(), debug_macros).unwrap();
    let mut query_cursor = QueryCursor::new();
    let matches = query_cursor.matches(&query, root_node, source.as_bytes());

    let mut matched = Vec::new();
    for m in matches {
        for capture in m.captures.iter() {
            match capture.node.kind() {
                "string_literal" => {
                    let range = capture.node.range();
                    let text = &source[range.start_byte..range.end_byte];
                    let line = range.start_point.row + 1;
                    let col = range.start_point.column;
                    let unquoted = &source[range.start_byte + 1..range.end_byte - 1];
                    let mut replaced = unquoted.replace("{}", "(\\w+)");
                    replaced = replaced.replace("{:?}", "(\\w+)");
                    let matcher = Regex::new(&replaced).unwrap();
                    let vars = Vec::new();
                    let name = find_fn_name(&capture.node, source);
                    let result = SourceRef {
                        line_no: line,
                        column: col,
                        name,
                        text,
                        matcher,
                        vars,
                    };
                    matched.push(result);
                }
                "identifier" => {
                    let range = capture.node.range();
                    let text = &source[range.start_byte..range.end_byte];
                    if text != "debug" {
                        let length = matched.len() - 1;
                        let prior_result: &mut SourceRef<'_> = matched.get_mut(length).unwrap();
                        prior_result.vars.push(&text);
                    }
                }
                _ => {
                    println!("ignoring {}", capture.node.kind())
                }
            }
        }
    }
    matched
}

fn find_fn_name<'a>(node: &Node, source: &'a str) -> &'a str {
    match node.kind() {
        "function_item" => {
            let range = node.child_by_field_name("name").unwrap().range();
            &source[range.start_byte..range.end_byte]
        },
        _ => {
            find_fn_name(&node.parent().unwrap(), source)
        }
    }
}
