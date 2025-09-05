#[cfg(test)]
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;
#[cfg(test)]
use std::path::PathBuf;
#[cfg(test)]
use std::ptr;

mod call_graph;
mod code_source;
mod log_format;
mod source_query;
mod source_ref;

// TODO: doesn't need to be exposed if we can clean up the arguments to do_mapping
use crate::source_ref::FormatArgument;
pub use call_graph::CallGraph;
use call_graph::Edge;
pub use code_source::CodeSource;
use log_format::LogFormat;
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

#[derive(Debug, PartialEq, Copy, Clone)]
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
                    (macro_invocation macro: (identifier)
                        (token_tree
                            (string_literal) @log
                        )
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
    pub variables: BTreeMap<String, String>,
    pub stack: Vec<Vec<SourceRef>>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LogRef<'a> {
    pub line: &'a str,
    details: Option<LogDetails<'a>>,
}

impl<'a> LogRef<'a> {
    pub fn body(self) -> &'a str {
        if let Some(LogDetails { body: Some(s), .. }) = self.details {
            s
        } else {
            self.line
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
struct LogDetails<'a> {
    file: Option<&'a str>,
    lineno: Option<u32>,
    body: Option<&'a str>,
}

impl<'a> LogRef<'a> {
    pub fn new(line: &'a str) -> Self {
        Self {
            line,
            details: None,
        }
    }

    pub fn with_format(line: &'a str, log_format: LogFormat) -> Self {
        let captures = log_format.captures(line);
        let file = captures.name("file").map(|file_match| file_match.as_str());
        let lineno = captures
            .name("line")
            .and_then(|lineno| lineno.as_str().parse::<u32>().ok());
        let body = captures.name("body").map(|body| body.as_str());
        Self {
            line,
            details: Some(LogDetails { file, lineno, body }),
        }
    }
}

pub fn link_to_source<'a>(log_ref: &LogRef, src_refs: &'a [SourceRef]) -> Option<&'a SourceRef> {
    src_refs
        .iter()
        .find(|&source_ref| source_ref.captures(log_ref.body()).is_some())
}

pub fn lookup_source<'a>(
    log_ref: &LogRef,
    log_format: &LogFormat,
    src_refs: &'a [SourceRef],
) -> Option<&'a SourceRef> {
    let captures = log_format.captures(log_ref.body());
    let file_name = captures.name("file").map_or("", |m| m.as_str());
    let line_no: usize = captures
        .name("line")
        .map_or(0, |m| m.as_str().parse::<usize>().unwrap_or_default());
    // println!("{:?} {:?}", file_name, line_no);

    src_refs.iter().find(|&source_ref| {
        // println!("source_ref.source_path = {} line_no = {}", source_ref.source_path, source_ref.line_no);
        source_ref.source_path.contains(file_name) && source_ref.line_no == line_no
    })
}

pub fn extract_variables<'a>(
    log_ref: LogRef<'a>,
    src_ref: &'a SourceRef,
) -> BTreeMap<String, String> {
    let mut variables = BTreeMap::new();
    let line = match log_ref.details {
        Some(details) => details.body.unwrap_or(log_ref.line),
        None => log_ref.line,
    };
    if let Some(captures) = src_ref.captures(line) {
        for (index, (cap, placeholder)) in
            std::iter::zip(captures.iter().skip(1), src_ref.args.iter()).enumerate()
        {
            let key = match placeholder {
                FormatArgument::Named(name) => name.clone(),
                FormatArgument::Positional(pos) => src_ref
                    .vars
                    .get(*pos)
                    .map(|s| s.as_str())
                    .unwrap_or("<unknown>")
                    .to_string(),
                FormatArgument::Placeholder => src_ref.vars[index].to_string(),
            };
            variables.insert(key, cap.unwrap().as_str().to_string());
        }
    }

    variables
}

pub fn filter_log(buffer: &str, filter: Filter, log_format: Option<String>) -> Vec<LogRef> {
    let log_format = log_format.map(LogFormat::new);
    buffer
        .lines()
        .enumerate()
        .filter_map(|(line_no, line)| {
            if filter.start <= line_no && line_no < filter.end {
                match &log_format {
                    Some(format) => Some(LogRef::with_format(line, format.clone())),
                    None => Some(LogRef::new(line)),
                }
            } else {
                None
            }
        })
        .collect()
}

pub fn do_mappings<'a>(
    log_refs: Vec<LogRef<'a>>,
    sources: &str,
    log_format: Option<String>,
) -> Vec<LogMapping<'a>> {
    let log_format = log_format.map(LogFormat::new);
    let source_filter = log_format
        .clone()
        .and_then(|f| f.build_src_filter(&log_refs));
    let mut sources = CodeSource::find_code(sources, source_filter);
    let src_logs = extract_logging(&mut sources);
    let call_graph = CallGraph::new(&mut sources);
    let use_hints = log_format.clone().is_some_and(|f| f.has_src_hint());

    log_refs
        .into_iter()
        .map(|log_ref| {
            let src_ref = if use_hints {
                lookup_source(&log_ref, log_format.as_ref().unwrap(), &src_logs)
            } else {
                link_to_source(&log_ref, &src_logs)
            };
            let variables = src_ref.as_ref().map_or(BTreeMap::new(), move |src_ref| {
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

pub fn extract_logging(sources: &mut [CodeSource]) -> Vec<SourceRef> {
    let mut matched = Vec::new();
    for code in sources.iter_mut() {
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
                        prior_result.vars.push(text.trim().to_string());
                    }
                }
                _ => println!("ignoring {}", result.kind),
            }
            // println!("*****");
        }
    }
    matched
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_log_defaults() {
        let buffer = String::from("hello\nwarning\nerror\nboom");
        let result = filter_log(&buffer, Filter::default(), None);
        assert_eq!(
            result,
            vec![
                LogRef::new("hello"),
                LogRef::new("warning"),
                LogRef::new("error"),
                LogRef::new("boom"),
            ]
        );
    }

    #[test]
    fn test_filter_log_with_filter() {
        let buffer = String::from("hello\nwarning\nerror\nboom");
        let result = filter_log(&buffer, Filter { start: 1, end: 2 }, None);
        assert_eq!(result, vec![LogRef::new("warning")]);
    }

    #[test]
    fn test_filter_log_with_format() {
        let buffer = String::from(
            "2025-04-10 22:12:52 INFO  JvmPauseMonitor:146 - JvmPauseMonitor-n0: Started",
        );
        let regex = String::from(
            r"^(?<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) (?<level>\w+)\s+ (?<file>[\w$.]+):(?<line>\d+) - (?<body>.*)$",
        );
        let log_format = Some(regex);
        let result = filter_log(&buffer, Filter::default(), log_format);
        let details = Some(LogDetails {
            file: Some("JvmPauseMonitor"),
            lineno: Some(146),
            body: Some("JvmPauseMonitor-n0: Started"),
        });
        assert_eq!(
            result,
            vec![LogRef {
                line: "2025-04-10 22:12:52 INFO  JvmPauseMonitor:146 - JvmPauseMonitor-n0: Started",
                details
            }]
        );
    }

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

fn nope(i: u32, j: i32) {
    debug!("this won't match i={}; j={}", i, j);
}

fn namedarg(name: &str) {
    debug!("Hello, {name}!");
}
    "#;

    #[test]
    fn test_extract_logging() {
        let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
        let src_refs = extract_logging(&mut [code]);
        assert_eq!(src_refs.len(), 3);
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
        assert_eq!(second.text, "\"this won't match i={}; j={}\"");
        assert_eq!(second.vars[0], "i");
    }

    #[test]
    fn test_link_to_source() {
        let lf = LogFormat::new(
            r#"^\[\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z \w+ \w+\]\s+(?<body>.*)"#.to_string(),
        );
        let log_ref = LogRef::with_format(
            "[2024-05-09T19:58:53Z DEBUG main] you're only as funky as your last cut",
            lf,
        );
        let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
        let src_refs = extract_logging(&mut [code]);
        assert_eq!(src_refs.len(), 3);
        let result = link_to_source(&log_ref, &src_refs);
        assert!(ptr::eq(result.unwrap(), &src_refs[0]));
    }

    #[test]
    fn test_link_to_source_no_matches() {
        let log_ref = LogRef::new("nope!");
        let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
        let src_refs = extract_logging(&mut [code]);
        assert_eq!(src_refs.len(), 3);
        let result = link_to_source(&log_ref, &src_refs);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_variables() {
        let log_ref = LogRef::new("this won't match i=1; j=2");
        let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
        let src_refs = extract_logging(&mut [code]);
        assert_eq!(src_refs.len(), 3);
        let vars = extract_variables(log_ref, &src_refs[1]);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars.get("i").map(|val| val.as_str()), Some("1"));
        assert_eq!(vars.get("j").map(|val| val.as_str()), Some("2"));
    }

    #[test]
    fn test_extract_named() {
        let log_ref = LogRef::new("Hello, Tim!");
        let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
        let src_refs = extract_logging(&mut [code]);
        assert_eq!(src_refs.len(), 3);
        let vars = extract_variables(log_ref, &src_refs[2]);
        assert_eq!(vars.get("name").map(|val| val.as_str()), Some("Tim"));
    }

    const TEST_PUNC_SRC: &str = r#"""
  private void run() {
    LOG.info("{}: Started", this);
    try {
      for (; Thread.currentThread().equals(threadRef.get()); ) {
        detectPause();
      }
    } finally {
      LOG.info("{}: Stopped", this);
    }
  }
"""#;
    #[test]
    fn test_extract_var_punctuation() {
        let regex = String::from(
            r"^(?<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) (?<level>\w+)\s+ (?<file>[\w$.]+):(?<line>\d+) - (?<body>.*)$",
        );
        let log_format = LogFormat::new(regex);
        let log_ref = LogRef::with_format(
            "2025-04-10 22:12:52 INFO  JvmPauseMonitor:146 - JvmPauseMonitor-n0: Started",
            log_format,
        );
        let code = CodeSource::new(
            PathBuf::from("in-mem.java"),
            Box::new(TEST_PUNC_SRC.as_bytes()),
        );
        let src_refs = extract_logging(&mut [code]);
        assert_eq!(src_refs.len(), 2);
        let vars = extract_variables(log_ref, &src_refs[0]);
        assert_eq!(
            vars.get("this").map(|val| val.as_str()),
            Some("JvmPauseMonitor-n0")
        );
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
            args: vec![],
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
            args: vec![],
            vars: vec![],
        };
        assert_eq!(paths, vec![vec![foo_2_nope, main_2_foo]])
    }
}
