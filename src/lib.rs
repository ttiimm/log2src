use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::ptr;

mod call_graph;
mod code_source;
mod source_query;
mod source_ref;

// TODO: doesn't need to be exposed if we can clean up the arguments to do_mapping
pub use call_graph::CallGraph;
use call_graph::Edge;
pub use code_source::CodeSource;
use source_query::QueryResult;
pub use source_query::SourceQuery;
pub use source_ref::SourceRef;

pub struct Filter {
    pub start: usize,
    pub end: usize,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            start: 0,
            end: usize::MAX,
        }
    }
}

#[derive(Debug, PartialEq)]
enum SourceLanguage {
    Rust,
    Java,
}

const IDENTS_RS: &[&str] = &["debug", "info", "warn"];
const IDENTS_JAVA: &[&str] = &["logger", "log", "fine", "debug", "info", "warn", "trace"];

impl SourceLanguage {
    fn get_query(&self) -> &str {
        match self {
            SourceLanguage::Rust => {
                // XXX: assumes it's a debug macro
                r#"
                    (macro_invocation macro: (identifier) @macro-name
                        (token_tree
                            (string_literal) @log (identifier)* @arguments
                        ) (#eq? @macro-name "debug")
                    )
                "#
            }
            SourceLanguage::Java => {
                r#"
                    (method_invocation 
                        object: (identifier) @object-name
                        name: (identifier) @method-name
                        arguments: (argument_list [
                            (_ (string_literal) @log  (_ (this)? @this (identifier) @arguments))
                            (_ (string_literal (_ (this)? @this (identifier) @arguments)) @log)
                            (string_literal) @log (this)? @this (identifier) @arguments
                            (string_literal) @log (this)? @this
                        ])
                        (#match? @object-name "log(ger)?|LOG(GER)?")
                        (#match? @method-name "fine|debug|info|warn|trace")
                    )
                "#
            }
        }
    }

    fn get_identifiers(&self) -> &[&str] {
        match self {
            SourceLanguage::Rust => IDENTS_RS,
            SourceLanguage::Java => IDENTS_JAVA,
        }
    }
}

#[derive(Serialize)]
pub struct LogMapping<'a> {
    #[serde(skip_serializing)]
    pub log_ref: LogRef<'a>,
    #[serde(rename(serialize = "srcRef"))]
    pub src_ref: Option<SourceRef>,
    pub variables: HashMap<String, String>,
    pub stack: Vec<Vec<SourceRef>>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LogRef<'a> {
    pub line: &'a str,
}

pub fn link_to_source<'a>(log_ref: &LogRef, src_refs: &'a [SourceRef]) -> Option<&'a SourceRef> {
    src_refs
        .iter()
        .find(|&source_ref| source_ref.captures(log_ref).is_some())
}

pub fn extract_variables<'a>(
    log_line: LogRef<'a>,
    src_ref: &'a SourceRef,
) -> HashMap<String, String> {
    let mut variables = HashMap::new();
    if src_ref.vars.len() > 0 {
        if let Some(captures) = src_ref.captures(&log_line) {
            for i in 0..captures.len() - 1 {
                variables.insert(
                    src_ref.vars[i].to_string(),
                    captures.get(i + 1).unwrap().as_str().to_string(),
                );
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
                Some(LogRef { line })
            } else {
                None
            }
        })
        .collect();
    results
}

pub fn do_mappings<'a>(log_refs: Vec<LogRef<'a>>, sources: &str) -> Vec<LogMapping<'a>> {
    let mut sources = CodeSource::find_code(sources);
    let src_logs = extract_logging(&mut sources);
    let call_graph = CallGraph::new(&mut sources);

    log_refs
        .into_iter()
        .map(|log_ref| {
            let src_ref: Option<&SourceRef> = link_to_source(&log_ref, &src_logs);
            let variables = src_ref.as_ref().map_or(HashMap::new(), move |src_ref| {
                extract_variables(log_ref, src_ref)
            });
            let stack = src_ref.as_ref().map_or(Vec::new(), |src_ref| {
                find_possible_paths(src_ref, &call_graph)
            });
            LogMapping {
                log_ref,
                src_ref: src_ref.cloned(),
                variables,
                stack,
            }
        })
        .collect::<Vec<LogMapping>>()
}

pub fn find_possible_paths<'a>(
    src_ref: &'a SourceRef,
    call_graph: &'a CallGraph,
) -> Vec<Vec<SourceRef>> {
    let mut possible = Vec::new();
    let mains = call_graph
        .edges
        .iter()
        .filter(|edge| edge.via.name == "main")
        .collect::<Vec<&Edge>>();
    for main in mains.into_iter() {
        let mut stack = vec![main];
        let mut visited = vec![main];
        while let Some(next) = stack.pop() {
            if next.to == src_ref.name {
                break;
            }

            let candidates = call_graph
                .edges
                .iter()
                .filter(|edge| next.to == edge.via.name)
                .collect::<Vec<&Edge>>();

            for edge in candidates {
                if !visited.contains(&edge) {
                    stack.push(edge);
                    visited.push(edge);
                }
            }
        }
        possible.push(
            visited
                .iter()
                .rev()
                .map(|edge| &edge.via)
                .cloned()
                .collect::<Vec<SourceRef>>(),
        );
    }

    possible
}

pub fn extract_logging<'a>(sources: &mut Vec<CodeSource>) -> Vec<SourceRef> {
    let mut matched = Vec::new();
    for code in sources.iter() {
        let src_query = SourceQuery::new(code);
        let query = code.language.get_query();
        let results = src_query.query(query, None);
        for result in results {
            // println!("node.kind()={:?} range={:?}", result.kind, result.range);
            match result.kind.as_str() {
                "string_literal" => {
                    let src_ref = SourceRef::new(code, result);
                    matched.push(src_ref);
                }
                "identifier" | "this" => {
                    let range = result.range;
                    let source = code.buffer.as_str();
                    let text = source[range.start_byte..range.end_byte].to_string();
                    // println!("text={} matched.len()={}", text, matched.len());
                    // check the text doesn't match any of the logging related identifiers
                    if code
                        .language
                        .get_identifiers()
                        .iter()
                        .all(|&s| s != text.to_lowercase())
                    {
                        let length = matched.len() - 1;
                        let prior_result: &mut SourceRef = matched.get_mut(length).unwrap();
                        prior_result.vars.push(text);
                    }
                }
                _ => println!("ignoring {}", result.kind),
            }
            // println!("*****");
        }
    }
    matched
}

#[test]
fn test_filter_log_defaults() {
    let buffer = String::from("hello\nwarning\nerror\nboom");
    let result = filter_log(&buffer, Filter::default());
    assert_eq!(
        result,
        vec![
            LogRef { line: "hello" },
            LogRef { line: "warning" },
            LogRef { line: "error" },
            LogRef { line: "boom" }
        ]
    );
}

#[test]
fn test_filter_log_with_filter() {
    let buffer = String::from("hello\nwarning\nerror\nboom");
    let result = filter_log(&buffer, Filter { start: 1, end: 2 });
    assert_eq!(result, vec![LogRef { line: "warning" }]);
}

#[cfg(test)]
const TEST_SOURCE: &str = r#"
#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    debug!("you're only as funky as your last cut");
    for i in 0..3 {
        foo(i);
    }
}

fn foo(i: u32) {
    nope(i);
}

fn nope(i: u32) {
    debug!("this won't match i={}", i);
}
"#;

#[test]
fn test_extract_logging() {
    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let src_refs = extract_logging(&mut vec![code]);
    assert_eq!(src_refs.len(), 2);
    let first = &src_refs[0];
    assert_eq!(first.line_no, 7);
    assert_eq!(first.column, 11);
    assert_eq!(first.name, "main");
    assert_eq!(first.text, "\"you're only as funky as your last cut\"");
    assert!(first.vars.is_empty());

    let second = &src_refs[1];
    assert_eq!(second.line_no, 18);
    assert_eq!(second.column, 11);
    assert_eq!(second.name, "nope");
    assert_eq!(second.text, "\"this won't match i={}\"");
    assert_eq!(second.vars[0], "i");
}

#[test]
fn test_link_to_source() {
    let log_ref = LogRef {
        line: "[2024-02-15T03:46:44Z DEBUG stack] you're only as funky as your last cut",
    };
    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let src_refs = extract_logging(&mut vec![code]);
    assert_eq!(src_refs.len(), 2);
    let result = link_to_source(&log_ref, &src_refs);
    assert!(ptr::eq(result.unwrap(), &src_refs[0]));
}

#[test]
fn test_link_to_source_no_matches() {
    let log_ref = LogRef {
        line: "[2024-02-26T03:44:40Z DEBUG stack] nope!",
    };

    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let src_refs = extract_logging(&mut vec![code]);
    assert_eq!(src_refs.len(), 2);
    let result = link_to_source(&log_ref, &src_refs);
    assert_eq!(result.is_none(), true);
}

#[test]
fn test_extract_variables() {
    let log_ref = LogRef {
        line: "[2024-02-15T03:46:44Z DEBUG nope] this won't match i=1",
    };
    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let src_refs = extract_logging(&mut vec![code]);
    assert_eq!(src_refs.len(), 2);
    let vars = extract_variables(log_ref, &src_refs[1]);
    assert_eq!(vars.get("i").map(|val| val.as_str()), Some("1"));
}

#[test]
fn test_call_graph() {
    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let mut sources = vec![code];
    let call_graph = CallGraph::new(&mut sources);
    let star_regex = Regex::new(".*").unwrap();
    let main_2_foo = SourceRef {
        source_path: String::from("in-mem.rs"),
        line_no: 9,
        column: 8,
        name: String::from("main"),
        text: String::from("foo"),
        matcher: star_regex,
        vars: vec![],
    };
    let star_regex = Regex::new(".*").unwrap();
    let foo_2_nope = SourceRef {
        source_path: String::from("in-mem.rs"),
        line_no: 14,
        column: 4,
        name: String::from("foo"),
        text: String::from("nope"),
        matcher: star_regex,
        vars: vec![],
    };
    assert_eq!(
        call_graph.edges,
        vec![
            Edge {
                to: "foo",
                via: main_2_foo
            },
            Edge {
                to: "nope",
                via: foo_2_nope
            }
        ]
    )
}

#[test]
fn test_find_possible_paths() {
    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let mut sources = vec![code];
    let src_refs = extract_logging(&mut sources);
    let call_graph = CallGraph::new(&mut sources);
    let paths = find_possible_paths(&src_refs[1], &call_graph);

    let star_regex = Regex::new(".*").unwrap();
    let main_2_foo = SourceRef {
        source_path: String::from("in-mem.rs"),
        line_no: 9,
        column: 8,
        name: String::from("main"),
        text: String::from("foo"),
        matcher: star_regex,
        vars: vec![],
    };
    let star_regex = Regex::new(".*").unwrap();
    let foo_2_nope = SourceRef {
        source_path: String::from("in-mem.rs"),
        line_no: 14,
        column: 4,
        name: String::from("foo"),
        text: String::from("nope"),
        matcher: star_regex,
        vars: vec![],
    };
    assert_eq!(paths, vec![vec![foo_2_nope, main_2_foo]])
}
