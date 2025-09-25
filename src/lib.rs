use itertools::Itertools;
use miette::Diagnostic;
use rayon::prelude::*;
use regex::{Captures, Regex, RegexSet};
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::ops::{Deref, RangeBounds};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use thiserror::Error;
use tree_sitter::Language;

mod code_source;
mod log_format;
mod progress;
mod source_hier;
mod source_query;
mod source_ref;

// TODO: doesn't need to be exposed if we can clean up the arguments to do_mapping
use crate::progress::WorkGuard;
use crate::source_hier::{ScanEvent, SourceFileID, SourceHierContent, SourceHierTree};
use crate::source_ref::FormatArgument;
pub use code_source::CodeSource;
use log_format::LogFormat;
pub use progress::ProgressTracker;
pub use progress::ProgressUpdate;
pub use progress::WorkInfo;
use source_query::QueryResult;
pub use source_query::SourceQuery;
pub use source_ref::SourceRef;

#[derive(Error, Debug, Diagnostic, Clone)]
pub enum LogError {
    #[error("\"{path}\" is already covered by \"{root}\"")]
    PathExists { path: PathBuf, root: PathBuf },
    #[error("cannot read source file \"{path}\"")]
    #[diagnostic(severity(warning))]
    CannotReadSourceFile {
        path: PathBuf,
        source: Arc<io::Error>,
    },
    #[error("no log statements found")]
    #[diagnostic(help(
        "\
    Make sure the source path is valid and refers to a tree with \
    supported source code and logging statements"
    ))]
    NoLogStatements,
    #[error("cannot access path \"{path}\"")]
    #[diagnostic(severity(warning))]
    CannotAccessPath {
        path: PathBuf,
        source: Arc<io::Error>,
    },
    #[error("unsupported file type \"{name}\"")]
    UnsupportedFileType { name: String },
}

/// Collection of log statements in a single source file
#[derive(Debug)]
pub struct StatementsInFile {
    pub path: String,
    id: SourceFileID,
    pub log_statements: Vec<SourceRef>,
    /// A single matcher for all log statements.
    /// XXX If there are too many in the file, the RegexSet constructor
    /// will fail with CompiledTooBig. We should probably fall back to
    /// manually trying each one at that point...
    pub matcher: RegexSet,
}

/// Collection of individual source files under a root path
pub struct SourceTree {
    pub tree: SourceHierTree,
    pub files_with_statements: HashMap<SourceFileID, StatementsInFile>,
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
            .all(|(_path, coll)| coll.files_with_statements.is_empty())
    }

    /// Add a source root path
    pub fn add_root(&mut self, path: &Path) -> Result<(), LogError> {
        if let Some(_existing_path) = self.match_path(path) {
        } else {
            self.roots
                .entry(path.to_owned())
                .or_insert_with(|| SourceTree {
                    tree: SourceHierTree::from(&path),
                    files_with_statements: HashMap::new(),
                });
        }
        Ok(())
    }

    /// Check if the given path is covered by any of the roots in this matcher.
    pub fn match_path(&self, path: &Path) -> Option<(&PathBuf, &SourceTree)> {
        self.roots
            .iter()
            .filter(|(existing_path, _coll)| path.starts_with(existing_path))
            .next()
    }

    pub fn find_source_file_statements(&self, path: &Path) -> Option<&StatementsInFile> {
        if let Some((_root_path, src_tree)) = self.match_path(path) {
            src_tree
                .tree
                .find_file(path)
                .and_then(|info| src_tree.files_with_statements.get(&info.id))
        } else {
            None
        }
    }

    /// Traverse the roots looking for supported source files.
    #[must_use]
    pub fn discover_sources(&mut self, tracker: &ProgressTracker) -> Vec<LogError> {
        tracker.begin_step("Finding source code".to_string());
        let pguard = tracker.doing_work(self.roots.len() as u64, "paths".to_string());
        self.roots.par_iter_mut().for_each(|(_path, coll)| {
            coll.tree.sync();
            pguard.inc(1);
        });
        let mut retval: Vec<LogError> = Vec::new();
        let mut file_count: usize = 0;
        self.roots.values().for_each(|coll| {
            coll.tree.visit(|node| match &node.content {
                SourceHierContent::File { .. } => file_count += 1,
                SourceHierContent::UnsupportedFile { .. } => {}
                SourceHierContent::Directory { .. } => {}
                SourceHierContent::Error { ref source } => retval.push(source.clone()),
                SourceHierContent::Unknown { .. } => {}
            });
        });
        tracker.end_step(format!("{} files found", file_count));

        retval
    }

    /// Scan the source files looking for potential log statements.
    pub fn extract_log_statements(&mut self, tracker: &ProgressTracker) {
        tracker.begin_step("Extracting log statements".to_string());
        self.roots.iter_mut().for_each(|(_path, coll)| {
            let guard = tracker.doing_work(coll.tree.stats().files as u64, "files".to_string());
            for event_chunk in &coll.tree.scan().chunks(10) {
                let sources = event_chunk
                    .flat_map(|event| match event {
                        ScanEvent::NewFile(path, info) => match File::open(&path) {
                            Ok(file) => match CodeSource::new(&path, info, file) {
                                Ok(cs) => Some(cs),
                                Err(_) => todo!(),
                            },
                            Err(_) => {
                                todo!()
                            }
                        },
                        ScanEvent::DeletedFile(_path, id) => {
                            coll.files_with_statements.remove(&id);
                            None
                        }
                    })
                    .collect::<Vec<CodeSource>>();
                extract_logging_guarded(&sources, &guard)
                    .into_iter()
                    .for_each(|sif| {
                        coll.files_with_statements.insert(sif.id, sif);
                    });
            }
        });
        tracker.end_step(format!(
            "{} found",
            self.roots
                .iter()
                .flat_map(|(_path, coll)| coll.files_with_statements.values())
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
                coll.files_with_statements
                    .values()
                    .filter(|stmts| stmts.path.contains(filename))
                    .flat_map(|stmts| {
                        let file_matches = stmts.matcher.matches(body);
                        match file_matches.iter().next() {
                            None => None,
                            Some(index) => stmts.log_statements.get(index),
                        }
                    })
                    .collect::<Vec<&SourceRef>>()
            } else {
                coll.files_with_statements
                    .par_iter()
                    .flat_map(|src_ref_coll| {
                        let file_matches = src_ref_coll.1.matcher.matches(log_ref.body());
                        match file_matches.iter().next() {
                            None => None,
                            Some(index) => src_ref_coll.1.log_statements.get(index),
                        }
                    })
                    .collect::<Vec<&SourceRef>>()
            };
            if let Some(src_ref) = matches
                .iter()
                .sorted_by(|lhs, rhs| rhs.quality.cmp(&lhs.quality))
                .next()
            {
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

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize)]
pub enum SourceLanguage {
    Rust,
    Java,
    #[serde(rename = "C++")]
    Cpp,
    Python,
}

impl From<SourceLanguage> for Language {
    fn from(value: SourceLanguage) -> Self {
        match value {
            SourceLanguage::Rust => tree_sitter_rust_orchard::LANGUAGE.into(),
            SourceLanguage::Java => tree_sitter_java::LANGUAGE.into(),
            SourceLanguage::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            SourceLanguage::Python => tree_sitter_python::LANGUAGE.into(),
        }
    }
}

const IDENTS_RS: &[&str] = &["debug", "info", "warn"];
const IDENTS_JAVA: &[&str] = &["logger", "log", "fine", "debug", "info", "warn", "trace"];
const IDENTS_CPP: &[&str] = &["debug", "info", "warn", "trace"];

const IDENTS_PYTHON: &[&str] = &["debug", "info", "warn", "trace"];

static RUST_PLACEHOLDER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{(?:([a-zA-Z_][a-zA-Z0-9_.]*)|(\d+))?\s*(?::[^}]*)?}"#).unwrap()
});

static JAVA_PLACEHOLDER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\{[^}]*}|\\\{([^}]*)}"#).unwrap());

static CPP_PLACEHOLDER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"%[-+ #0]*\d*(?:\.\d+)?[hlLzjt]*[diuoxXfFeEgGaAcspn%]|\{(?:([a-zA-Z_][a-zA-Z0-9_.]*)|(\d+))?\s*(?::[^}]*)?}"#).unwrap()
});

static PYTHON_PLACEHOLDER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"%[-+ #0]*\d*(?:\.\d+)?[hlLzjt]*[diuoxXfFeEgGaAcspn%]"#).unwrap()
});

impl SourceLanguage {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceLanguage::Rust => "Rust",
            SourceLanguage::Java => "Java",
            SourceLanguage::Cpp => "C++",
            SourceLanguage::Python => "Python",
        }
    }

    fn from_extension(extension: &OsStr) -> Option<Self> {
        match extension.to_str() {
            Some("rs") => Some(Self::Rust),
            Some("java") => Some(Self::Java),
            Some("h" | "hh" | "hpp" | "hxx" | "tpp" | "cc" | "cpp" | "cxx") => Some(Self::Cpp),
            Some("py") => Some(Self::Python),
            None | Some(_) => None,
        }
    }

    fn from_path(path: &Path) -> Option<Self> {
        match path.extension() {
            Some(extension) => Self::from_extension(extension),
            // Some languages might have well-known file names without an extension
            None => None,
        }
    }

    fn get_query(&self) -> &str {
        match self {
            SourceLanguage::Rust => {
                // XXX: assumes it's a debug macro
                r#"
                    (macro_invocation macro: (_) @macro-name
                        (token_tree .
                            (string_literal) @log
                        )
                        (#not-any-of? @macro-name "format" "vec")
                    )
                "#
            }
            SourceLanguage::Java => {
                r#"
                    (method_invocation 
                        object: (identifier) @object-name
                        name: (identifier) @method-name
                        arguments: [
                            (argument_list (template_expression
                                template_argument: (string_literal) @arguments))
                            (argument_list . (string_literal) @arguments)
                        ]
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
                                    function: (_) @fname
                                    arguments: (argument_list (string_literal) @arguments)
                                )
                            )
                        )
                        (#not-match? @fname "snprintf|sprintf")
                    )
                "#
            }
            SourceLanguage::Python => {
                r#"
                (
                    (expression_statement
                      (call
                        function: (_) @func
                        arguments: (argument_list .
                          (string) @args
                        )
                      )
                    )
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
            SourceLanguage::Python => IDENTS_PYTHON,
        }
    }

    fn get_placeholder_regex(&self) -> &'static Regex {
        match self {
            SourceLanguage::Rust => RUST_PLACEHOLDER_REGEX.deref(),
            SourceLanguage::Java => JAVA_PLACEHOLDER_REGEX.deref(),
            SourceLanguage::Cpp => CPP_PLACEHOLDER_REGEX.deref(),
            SourceLanguage::Python => PYTHON_PLACEHOLDER_REGEX.deref(),
        }
    }

    fn captures_to_format_arg(&self, caps: &Captures) -> FormatArgument {
        for (index, cap) in caps.iter().skip(1).enumerate() {
            if let Some(cap) = cap {
                return match (self, index) {
                    (SourceLanguage::Rust | SourceLanguage::Java | SourceLanguage::Cpp, 0) => {
                        FormatArgument::Named(cap.as_str().to_string())
                    }
                    (SourceLanguage::Rust | SourceLanguage::Cpp, 1) => {
                        FormatArgument::Positional(cap.as_str().parse().unwrap())
                    }
                    _ => unreachable!(),
                };
            }
        }
        FormatArgument::Placeholder
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
        .sorted_by(|lhs, rhs| rhs.quality.cmp(&lhs.quality))
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
        let mut placeholder_index = 0;
        for (cap, placeholder) in std::iter::zip(captures.iter().skip(1), src_ref.args.iter()) {
            let expr = match placeholder {
                FormatArgument::Named(name) => name.clone(),
                FormatArgument::Positional(pos) => src_ref
                    .vars
                    .get(*pos)
                    .map(|s| s.as_str())
                    .unwrap_or("<unknown>")
                    .to_string(),
                FormatArgument::Placeholder => {
                    let res = src_ref.vars[placeholder_index].to_string();

                    placeholder_index += 1;
                    res
                }
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

pub fn extract_logging_guarded(sources: &[CodeSource], guard: &WorkGuard) -> Vec<StatementsInFile> {
    sources
        .par_iter()
        .flat_map(|code| {
            let mut matched = vec![];
            let mut patterns = vec![];
            let src_query = SourceQuery::new(code);
            let query = code.info.language.get_query();
            let results = src_query.query(query, None);
            for result in results {
                // println!("node.kind()={:?} range={:?}", result.kind, result.range);
                match result.kind.as_str() {
                    "string_literal" | "string" => {
                        if let Some(src_ref) = SourceRef::new(code, result) {
                            patterns.push(src_ref.pattern.clone());
                            matched.push(src_ref);
                        }
                    }
                    "args" | "this" => {
                        if !matched.is_empty() {
                            let range = result.range;
                            let source = code.buffer.as_str();
                            let text = source[range.start_byte..range.end_byte].to_string();
                            // eprintln!("text={} matched.len()={}", text, matched.len());
                            // check the text doesn't match any of the logging related identifiers
                            if code
                                .info
                                .language
                                .get_identifiers()
                                .iter()
                                .all(|&s| s != text.to_lowercase())
                            {
                                let length = matched.len() - 1;
                                let prior_result: &mut SourceRef = matched.get_mut(length).unwrap();
                                prior_result.end_line_no = result.range.end_point.row + 1;
                                prior_result.vars.push(text.trim().to_string());
                            }
                        }
                    }
                    _ => {} // eprintln!("ignoring {}", result.kind),
                }
                // println!("*****");
            }
            guard.inc(1);
            if matched.is_empty() {
                None
            } else {
                Some(StatementsInFile {
                    path: matched.first().unwrap().source_path.clone(),
                    id: code.info.id,
                    log_statements: matched,
                    matcher: RegexSet::new(patterns).expect("To combine patterns"),
                })
            }
        })
        .collect()
}

pub fn extract_logging(sources: &[CodeSource], tracker: &ProgressTracker) -> Vec<StatementsInFile> {
    let guard = tracker.doing_work(sources.len() as u64, "files".to_string());
    extract_logging_guarded(sources, &guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_yaml_snapshot;
    use std::ptr;

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
    log::debug!("this won't match i={}; j={}", i, j);
}

fn namedarg0(salutation: &str, name: &str) {
    debug!("{salutation}, {name}!"); // lower quality than the next one
}

fn namedarg(name: &str) {
    let msg = format!("Goodbye, {name}!");
    debug!("Hello, {name}!");
}

fn namedarg2(salutation: &str, name: &str) {
    debug!("{salutation}, {name}!"); // lower quality than the previous one
}
    "#;

    #[test]
    fn test_extract_logging() {
        let code = CodeSource::from_string(&Path::new("in-mem.rs"), TEST_SOURCE);
        let src_refs = extract_logging(&[code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_yaml_snapshot!(src_refs);
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
        let code = CodeSource::from_string(&Path::new("in-mem.rs"), TEST_SOURCE);
        let src_refs = extract_logging(&[code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 5);
        let result = link_to_source(&log_ref, &src_refs);
        assert!(ptr::eq(result.unwrap(), &src_refs[0]));
    }

    #[test]
    fn test_link_to_quality_source() {
        let lf = LogFormat::new(
            r#"^\[\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z \w+ \w+\]\s+(?<body>.*)"#.to_string(),
        );
        let log_ref = LogRef::with_format("[2024-05-09T19:58:53Z DEBUG main] Hello, Leander!", lf);
        let code = CodeSource::from_string(&Path::new("in-mem.rs"), TEST_SOURCE);
        let src_refs = extract_logging(&[code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        let result = link_to_source(&log_ref, &src_refs);
        assert_yaml_snapshot!(result);
    }

    const MULTILINE_SOURCE: &str = r#"
#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    let adjective = "funky";
    debug!("you're only as {}\n as your last cut", adjective);
}
"#;
    #[test]
    fn test_link_multiline() {
        let lf = LogFormat::new(
            r#"^\[\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z \w+ \w+\]\s+(?<body>.*)"#.to_string(),
        );
        let log_ref = LogRef::with_format(
            "[2024-05-09T19:58:53Z DEBUG main] you're only as funky\n as your last cut",
            lf,
        );
        let code = CodeSource::from_string(&Path::new("in-mem.rs"), MULTILINE_SOURCE);
        let src_refs = extract_logging(&[code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 1);
        let result = link_to_source(&log_ref, &src_refs);
        assert!(ptr::eq(result.unwrap(), &src_refs[0]));
        let vars = extract_variables(&log_ref, &src_refs[0]);
        assert_eq!(
            vars,
            [VariablePair {
                expr: "adjective".to_string(),
                value: "funky".to_string()
            }]
        );
    }

    #[test]
    fn test_link_to_source_no_matches() {
        let log_ref = LogRef::new("nope!");
        let code = CodeSource::from_string(&Path::new("in-mem.rs"), TEST_SOURCE);
        let src_refs = extract_logging(&[code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 5);
        let result = link_to_source(&log_ref, &src_refs);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_variables() {
        let log_ref = LogRef::new("this won't match i=1; j=2");
        let code = CodeSource::from_string(&Path::new("in-mem.rs"), TEST_SOURCE);
        let src_refs = extract_logging(&[code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 5);
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
        let code = CodeSource::from_string(&Path::new("in-mem.rs"), TEST_SOURCE);
        let src_refs = extract_logging(&[code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_eq!(src_refs.len(), 5);
        let vars = extract_variables(&log_ref, &src_refs[3]);
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
        let code = CodeSource::from_string(&PathBuf::from("in-mem.java"), TEST_PUNC_SRC);
        let src_refs = extract_logging(&[code], &ProgressTracker::new())
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
        let code = CodeSource::from_string(&Path::new("in-mem.cc"), CPP_SOURCE);
        let src_refs = extract_logging(&[code], &ProgressTracker::new())
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

    const PYTHON_SOURCE: &str = r#"
def main(args):
    logger.info("foo %s \N{greek small letter pi}", test_var)
    logging.info(f'Hello, {args[1]}!')
    logger.warning(f"warning message:\nlow disk space")
    logger.info(rf"""info message:
processing \started -- {args[0]}""")
"#;

    #[test]
    fn test_basic_python() {
        let log_ref = LogRef::new("foo bar π");
        let code = CodeSource::from_string(&Path::new("in-mem.py"), PYTHON_SOURCE);
        let src_refs = extract_logging(&[code], &ProgressTracker::new())
            .pop()
            .unwrap()
            .log_statements;
        assert_yaml_snapshot!(src_refs);
        let vars = extract_variables(&log_ref, &src_refs[0]);
        assert_eq!(
            vars,
            vec![VariablePair {
                expr: "test_var".to_string(),
                value: "bar".to_string()
            },]
        );
    }
}
