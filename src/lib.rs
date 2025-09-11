use miette::Diagnostic;
use rayon::prelude::*;
use regex::RegexSet;
use serde::Serialize;
use std::collections::HashMap;
use std::io;
use std::ops::RangeBounds;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::ptr;
use thiserror::Error;

mod code_source;
mod log_format;
mod progress;
mod source_query;
mod source_ref;

// TODO: doesn't need to be exposed if we can clean up the arguments to do_mapping
use crate::source_ref::FormatArgument;
pub use code_source::CodeSource;
use log_format::LogFormat;
pub use progress::ProgressTracker;
pub use progress::ProgressUpdate;
use source_query::QueryResult;
pub use source_query::SourceQuery;
pub use source_ref::SourceRef;

#[derive(Error, Debug, Diagnostic)]
pub enum LogError {
    #[error("\"{path}\" is already covered by \"{root}\"")]
    PathExists { path: PathBuf, root: PathBuf },
    #[error("cannot read source file \"{path}\"")]
    #[diagnostic(severity(warning))]
    CannotReadSourceFile { path: PathBuf, source: io::Error },
    #[error("no log statements found")]
    #[diagnostic(help(
        "\
    Make sure the source path is valid and refers to a tree with \
    supported source code and logging statements"
    ))]
    NoLogStatements,
    #[error("cannot access path \"{path}\"")]
    #[diagnostic(severity(warning))]
    CannotAccessPath { path: PathBuf, source: io::Error },
}

/// Collection of log statements in a single source file
#[derive(Debug)]
pub struct StatementsInFile {
    pub filename: String,
    pub log_statements: Vec<SourceRef>,
    /// A single matcher for all log statements.
    /// XXX If there are too many in the file, the RegexSet constructor
    /// will fail with CompiledTooBig. We should probably fall back to
    /// manually trying each one at that point...
    pub matcher: RegexSet,
}

/// Collection of individual source files under a root path
pub struct SourceTree {
    pub sources: Vec<CodeSource>,
    pub statements: Vec<StatementsInFile>,
}

/// Collection of root paths to their tree of source files
/// that contain log statements.
pub struct LogMatcher {
    roots: HashMap<PathBuf, SourceTree>,
}

impl LogMatcher {
    /// Create an empty LogMatcher
    pub fn new() -> Self {
        Self {
            roots: HashMap::new(),
        }
    }

    /// True if no log statements are recognized by this matcher.
    pub fn is_empty(&self) -> bool {
        self.roots
            .iter()
            .all(|(_path, coll)| coll.statements.is_empty())
    }

    pub fn len(&self) -> usize {
        self.roots
            .iter()
            .map(|(_path, coll)| coll.sources.len())
            .sum()
    }

    /// Add a source root path
    pub fn add_root(&mut self, path: &Path) -> Result<(), LogError> {
        if let Some(existing_path) = self.match_path(path) {
            Err(LogError::PathExists {
                path: PathBuf::from(path),
                root: existing_path,
            })
        } else {
            self.roots
                .entry(path.to_owned())
                .or_insert_with(|| SourceTree {
                    sources: vec![],
                    statements: vec![],
                });
            Ok(())
        }
    }

    /// Check if the given path is covered by any of the roots in this matcher.
    pub fn match_path(&self, path: &Path) -> Option<PathBuf> {
        self.roots
            .iter()
            .filter(|(existing_path, _coll)| path.starts_with(existing_path))
            .map(|(path, _coll)| path.clone())
            .next()
    }

    /// Traverse the roots looking for supported source files.
    #[must_use]
    pub fn discover_sources(&mut self, tracker: &ProgressTracker) -> Vec<LogError> {
        tracker.begin_step("Finding source code".to_string());
        let pguard = tracker.doing_work(self.roots.len() as u64, "paths".to_string());
        let retval = self
            .roots
            .par_iter_mut()
            .filter(|(_path, coll)| coll.sources.is_empty())
            .flat_map(|(path, coll)| {
                let (srcs, errs) = CodeSource::find_code(path, None);
                coll.sources = srcs;
                pguard.inc(1);
                errs
            })
            .collect();
        tracker.end_step(format!(
            "{} files found",
            self.roots
                .iter()
                .map(|(_path, coll)| coll.sources.len())
                .sum::<usize>()
        ));

        retval
    }

    /// Scan the source files looking for potential log statements.
    pub fn extract_log_statements(&mut self, tracker: &ProgressTracker) {
        tracker.begin_step("Extracting log statements".to_string());
        self.roots.iter_mut().for_each(|(_path, coll)| {
            if coll.statements.is_empty() {
                coll.statements = extract_logging(&coll.sources, tracker);
            }
        });
        tracker.end_step(format!(
            "{} found",
            self.roots
                .iter()
                .flat_map(|(_path, coll)| coll.statements.iter())
                .map(|stmts| stmts.log_statements.len())
                .sum::<usize>()
        ));
    }

    /// Attempt to match the given log message.
    pub fn match_log_statement<'a>(&self, log_ref: &LogRef<'a>) -> Option<LogMapping<'a>> {
        for (_path, coll) in &self.roots {
            let matches = if let Some(LogDetails {
                file: Some(filename),
                body: Some(body),
                ..
            }) = log_ref.details
            {
                // XXX this block and the else are basically the same, try to refactor
                coll.statements
                    .iter()
                    .filter(|stmts| stmts.filename.contains(filename))
                    .flat_map(|stmts| {
                        let file_matches = stmts.matcher.matches(body);
                        match file_matches.iter().next() {
                            None => None,
                            Some(index) => stmts.log_statements.get(index),
                        }
                    })
                    .collect::<Vec<&SourceRef>>()
            } else {
                coll.statements
                    .par_iter()
                    .flat_map(|src_ref_coll| {
                        let file_matches = src_ref_coll.matcher.matches(log_ref.body());
                        match file_matches.iter().next() {
                            None => None,
                            Some(index) => src_ref_coll.log_statements.get(index),
                        }
                    })
                    .collect::<Vec<&SourceRef>>()
            };
            if let Some(src_ref) = matches.first() {
                let variables = extract_variables(log_ref, src_ref);
                return Some(LogMapping {
                    log_ref: log_ref.clone(),
                    src_ref: Some((*src_ref).clone()),
                    variables,
                });
            }
        }
        None
    }
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize)]
pub enum SourceLanguage {
    Rust,
    Java,
    #[serde(rename = "C++")]
    Cpp,
}

const IDENTS_RS: &[&str] = &["debug", "info", "warn"];
const IDENTS_JAVA: &[&str] = &["logger", "log", "fine", "debug", "info", "warn", "trace"];
const IDENTS_CPP: &[&str] = &["debug", "info", "warn", "trace"];

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
            SourceLanguage::Cpp => {
                r#"
                    (
                        (compound_statement
                            (expression_statement
                                (call_expression
                                    function: (identifier) @fname
                                    arguments: (argument_list (string_literal) @arguments)
                                )
                            )
                        )
                        (#not-match? @fname "snprintf|sprintf")
                    )
                "#
            }
        }
    }

    fn get_identifiers(&self) -> &[&str] {
        match self {
            SourceLanguage::Rust => IDENTS_RS,
            SourceLanguage::Java => IDENTS_JAVA,
            SourceLanguage::Cpp => IDENTS_CPP,
        }
    }
}

#[derive(PartialEq, Clone, Debug, Serialize)]
pub struct VariablePair {
    pub expr: String,
    pub value: String,
}

#[derive(Serialize)]
pub struct LogMapping<'a> {
    #[serde(skip_serializing)]
    pub log_ref: LogRef<'a>,
    #[serde(rename(serialize = "srcRef"))]
    pub src_ref: Option<SourceRef>,
    pub variables: Vec<VariablePair>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LogRef<'a> {
    pub line: &'a str,
    pub details: Option<LogDetails<'a>>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LogDetails<'a> {
    pub file: Option<&'a str>,
    pub lineno: Option<u32>,
    pub body: Option<&'a str>,
}

impl<'a> LogRef<'a> {
    pub fn new(line: &'a str) -> Self {
        Self {
            line,
            details: None,
        }
    }

    pub fn from_parsed(file: Option<&'a str>, lineno: Option<u32>, body: &'a str) -> Self {
        let details = Some(LogDetails {
            file,
            lineno,
            body: Some(body),
        });
        Self {
            line: body,
            details,
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

    pub fn body(self) -> &'a str {
        if let Some(LogDetails { body: Some(s), .. }) = self.details {
            s
        } else {
            self.line
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

pub fn extract_variables<'a>(log_ref: &LogRef<'a>, src_ref: &'a SourceRef) -> Vec<VariablePair> {
    let mut variables = Vec::new();
    let line = match log_ref.details {
        Some(details) => details.body.unwrap_or(log_ref.line),
        None => log_ref.line,
    };
    if let Some(captures) = src_ref.captures(line) {
        for (index, (cap, placeholder)) in
            std::iter::zip(captures.iter().skip(1), src_ref.args.iter()).enumerate()
        {
            let expr = match placeholder {
                FormatArgument::Named(name) => name.clone(),
                FormatArgument::Positional(pos) => src_ref
                    .vars
                    .get(*pos)
                    .map(|s| s.as_str())
                    .unwrap_or("<unknown>")
                    .to_string(),
                FormatArgument::Placeholder => src_ref.vars[index].to_string(),
            };
            variables.push(VariablePair {
                expr,
                value: cap.unwrap().as_str().to_string(),
            });
        }
    }

    variables
}

pub fn filter_log<R>(buffer: &str, filter: R, log_format: Option<String>) -> Vec<LogRef<'_>>
where
    R: RangeBounds<usize>,
{
    let log_format = log_format.map(LogFormat::new);
    buffer
        .lines()
        .enumerate()
        .filter_map(|(line_no, line)| {
            if filter.contains(&line_no) {
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

pub fn extract_logging(sources: &[CodeSource], tracker: &ProgressTracker) -> Vec<StatementsInFile> {
    let guard = tracker.doing_work(sources.len() as u64, "files".to_string());
    sources
        .par_iter()
        .flat_map(|code| {
            let mut matched = vec![];
            let mut patterns = vec![];
            let src_query = SourceQuery::new(code);
            let query = code.language.get_query();
            let results = src_query.query(query, None);
            for result in results {
                // println!("node.kind()={:?} range={:?}", result.kind, result.range);
                match result.kind.as_str() {
                    "string_literal" => {
                        if let Some(src_ref) = SourceRef::new(code, result) {
                            patterns.push(src_ref.pattern.clone());
                            matched.push(src_ref);
                        }
                    }
                    "identifier" | "this" => {
                        if !matched.is_empty() {
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
                    }
                    _ => println!("ignoring {}", result.kind),
                }
                // println!("*****");
            }
            guard.inc(1);
            if matched.is_empty() {
                None
            } else {
                Some(StatementsInFile {
                    filename: matched.first().unwrap().source_path.clone(),
                    log_statements: matched,
                    matcher: RegexSet::new(patterns).expect("To combine patterns"),
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_log_defaults() {
        let buffer = String::from("hello\nwarning\nerror\nboom");
        let result = filter_log(&buffer, .., None);
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
        let result = filter_log(&buffer, 1..2, None);
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
        let result = filter_log(&buffer, .., log_format);
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
        let code = CodeSource::new(
            &PathBuf::from("in-mem.rs"),
            Box::new(TEST_SOURCE.as_bytes()),
        )
        .unwrap();
        let src_refs = extract_logging(&mut [code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
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
        let code = CodeSource::new(
            &PathBuf::from("in-mem.rs"),
            Box::new(TEST_SOURCE.as_bytes()),
        )
        .unwrap();
        let src_refs = extract_logging(&mut [code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 3);
        let result = link_to_source(&log_ref, &src_refs);
        assert!(ptr::eq(result.unwrap(), &src_refs[0]));
    }

    #[test]
    fn test_link_to_source_no_matches() {
        let log_ref = LogRef::new("nope!");
        let code = CodeSource::new(
            &PathBuf::from("in-mem.rs"),
            Box::new(TEST_SOURCE.as_bytes()),
        )
        .unwrap();
        let src_refs = extract_logging(&mut [code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 3);
        let result = link_to_source(&log_ref, &src_refs);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_variables() {
        let log_ref = LogRef::new("this won't match i=1; j=2");
        let code = CodeSource::new(
            &PathBuf::from("in-mem.rs"),
            Box::new(TEST_SOURCE.as_bytes()),
        )
        .unwrap();
        let src_refs = extract_logging(&mut [code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 3);
        let vars = extract_variables(&log_ref, &src_refs[1]);
        assert_eq!(
            vars,
            vec![
                VariablePair {
                    expr: "i".to_string(),
                    value: "1".to_string()
                },
                VariablePair {
                    expr: "j".to_string(),
                    value: "2".to_string()
                }
            ]
        );
    }

    #[test]
    fn test_extract_named() {
        let log_ref = LogRef::new("Hello, Tim!");
        let code = CodeSource::new(
            &PathBuf::from("in-mem.rs"),
            Box::new(TEST_SOURCE.as_bytes()),
        )
        .unwrap();
        let src_refs = extract_logging(&mut [code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 3);
        let vars = extract_variables(&log_ref, &src_refs[2]);
        assert_eq!(
            vars,
            vec![VariablePair {
                expr: "name".to_string(),
                value: "Tim".to_string()
            },]
        );
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
            &PathBuf::from("in-mem.java"),
            Box::new(TEST_PUNC_SRC.as_bytes()),
        )
        .unwrap();
        let src_refs = extract_logging(&mut [code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 2);
        let vars = extract_variables(&log_ref, &src_refs[0]);
        assert_eq!(
            vars,
            vec![VariablePair {
                expr: "this".to_string(),
                value: "JvmPauseMonitor-n0".to_string()
            },]
        );
    }

    const CPP_SOURCE: &str = r#"
    #include <stdio.h>

    int main(int argc, char* argv[]) {
        printf("Hello, %s!", argv[1]);
    }
    "#;

    #[test]
    fn test_basic_cpp() {
        let log_ref = LogRef::new("Hello, Steve!");
        let code =
            CodeSource::new(&PathBuf::from("in-mem.cc"), Box::new(CPP_SOURCE.as_bytes())).unwrap();
        let src_refs = extract_logging(&mut [code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 1);
        let vars = extract_variables(&log_ref, &src_refs[0]);
        assert_eq!(
            vars,
            vec![VariablePair {
                expr: "argv[1]".to_string(),
                value: "Steve".to_string()
            },]
        );
    }
}
