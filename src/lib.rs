use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use tree_sitter::{Node, Parser, Query, QueryCapture, QueryCursor, Tree};

pub struct Filter {
    pub start: usize,
    pub end: usize,
}

impl Default for Filter {
    fn default() -> Self {
        Self { start: 0, end: usize::MAX }
    }
}

#[derive(Serialize)]
pub struct LogMapping<'a> {
    #[serde(skip_serializing)]
    pub log_ref: &'a LogRef<'a>,
    #[serde(rename(serialize = "srcRef"))]
    pub src_ref: Option<&'a SourceRef<'a>>,
    pub variables: HashMap<&'a str, &'a str>,
    pub stack: Vec<Vec<&'a SourceRef<'a>>>,
}

#[derive(Debug)]
#[derive(PartialEq)]
pub struct LogRef<'a> {
    pub text: &'a str,
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

#[derive(Debug)]
pub struct CallGraph<'a> {
    _nodes: Vec<&'a str>,
    edges: Vec<Edge<'a>>,
}

#[derive(Debug)]
pub struct Edge<'a> {
    from: &'a str,
    to: &'a str,
    via: SourceRef<'a>,
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
    src_ref: &'a SourceRef,
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

pub fn filter_log(buffer: &String, filter: Filter) -> Vec<LogRef> {
    let results = buffer
        .lines()
        .enumerate()
        .filter_map(|(line_no, line)| {
            if filter.start <= line_no && line_no < filter.end {
                Some(LogRef { text: line })
            } else {
                None
            }
        })
        .collect();
    results
}

pub fn find_possible_paths<'a>(
    src_ref: &'a SourceRef,
    call_graph: &'a CallGraph,
) -> Vec<Vec<&'a SourceRef<'a>>> {
    let mut possible = Vec::new();
    let mut visited = Vec::new();
    for main_edge in call_graph.edges.iter().filter(|e| e.from == "main") {
        let mut path = Vec::new();
        path.push(&main_edge.via);
        viable_path(
            main_edge.to,
            src_ref.name,
            call_graph,
            &mut possible,
            &mut visited,
            &mut path,
        );
    }
    possible
}

fn viable_path<'a>(
    node: &'a str,
    target: &'a str,
    call_graph: &'a CallGraph,
    possible: &mut Vec<Vec<&SourceRef<'a>>>,
    visited: &mut Vec<&'a str>,
    path: &mut Vec<&'a SourceRef<'a>>,
) {
    if visited.contains(&node) {
        return;
    }
    visited.push(node);

    if node == target {
        let mut found_path = path.to_vec();
        found_path.reverse();
        possible.push(found_path);
        return;
    }

    for next_edge in call_graph.edges.iter().filter(|e| e.from == node) {
        path.push(&next_edge.via);
        viable_path(next_edge.to, target, call_graph, possible, visited, path);
        path.pop();
    }
}

pub fn build_graph(source: &str) -> CallGraph {
    let tree = parse(source);
    let root_node = tree.root_node();
    let node_query = r#"
        (function_item name: (identifier) @fn_name parameters: (parameters)*)
    "#;
    let nodes = find_nodes(root_node, source, node_query);

    let edge_query = r#"
        (call_expression function: (identifier) @fn_name arguments: (arguments (_))*)
    "#;
    let edges = find_edges(root_node, source, edge_query);

    CallGraph {
        _nodes: nodes,
        edges,
    }
}

fn find_nodes<'a, 'b>(root_node: Node, source: &'a str, to_query: &'b str) -> Vec<&'a str> {
    let query = Query::new(tree_sitter_rust::language(), to_query).unwrap();
    let mut query_cursor = QueryCursor::new();
    let matches = query_cursor.matches(&query, root_node, source.as_bytes());
    let name_idx = query.capture_index_for_name("fn_name").unwrap();
    let mut symbols = Vec::new();
    for m in matches {
        for capture in m.captures.iter().filter(|c| c.index == name_idx) {
            let range = capture.node.range();
            let name = &source[range.start_byte..range.end_byte];
            symbols.push(name);
        }
    }
    symbols
}

fn find_edges<'a, 'b>(root_node: Node, source: &'a str, to_query: &'b str) -> Vec<Edge<'a>> {
    let query = Query::new(tree_sitter_rust::language(), to_query).unwrap();
    let mut query_cursor = QueryCursor::new();
    let matches = query_cursor.matches(&query, root_node, source.as_bytes());
    let name_idx = query.capture_index_for_name("fn_name").unwrap();
    let mut symbols = Vec::new();
    for m in matches {
        for capture in m.captures.iter().filter(|c| c.index == name_idx) {
            let range = capture.node.range();
            let fn_call = &source[range.start_byte..range.end_byte];
            let enclosing = find_fn_name(&capture.node, source);
            let src_ref = build_src_ref(&source, &capture);
            symbols.push(Edge {
                from: enclosing,
                to: fn_call,
                via: src_ref,
            });
        }
    }
    symbols
}

pub fn extract_source(source: &str) -> Vec<SourceRef> {
    let tree = parse(source);
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
                    let result = build_src_ref(source, capture);
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

fn build_src_ref<'a>(source: &'a str, capture: &QueryCapture<'_>) -> SourceRef<'a> {
    let range = capture.node.range();
    let text = &source[range.start_byte..range.end_byte];
    let line = range.start_point.row + 1;
    let col = range.start_point.column;
    let start = range.start_byte + 1;
    let mut end = range.end_byte - 1;
    if start == range.end_byte {
        end = range.end_byte;
    }
    let unquoted = &source[start..end];
    let mut replaced = unquoted.replace("{}", "(\\w+)");
    replaced = replaced.replace("{:?}", "(\\w+)");
    let matcher = Regex::new(&replaced).unwrap();
    let vars = Vec::new();
    let name = find_fn_name(&capture.node, source);
    SourceRef {
        line_no: line,
        column: col,
        name,
        text,
        matcher,
        vars,
    }
}

fn find_fn_name<'a>(node: &Node, source: &'a str) -> &'a str {
    match node.kind() {
        "function_item" => {
            let range = node.child_by_field_name("name").unwrap().range();
            &source[range.start_byte..range.end_byte]
        }
        _ => find_fn_name(&node.parent().unwrap(), source),
    }
}

fn parse(source: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(tree_sitter_rust::language())
        .expect("Error loading Rust grammar");
    parser.parse(&source, None).expect("source is parable")
}

#[test]
fn test_filter_log_defaults() {
    let buffer = String::from("hello\nwarning\nerror\nboom");
    let result = filter_log(&buffer, Filter::default());
    assert_eq!(result, vec![LogRef{text: "hello"}, LogRef{text: "warning"}, LogRef{text: "error"}, LogRef{text: "boom"}]);
}