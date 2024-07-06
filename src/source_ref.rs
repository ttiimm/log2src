use core::fmt;

use regex::{Captures, Regex};
use serde::Serialize;

use crate::{CodeSource, LogRef, QueryResult};

// TODO: get rid of this clone?
#[derive(Clone, Debug, Serialize)]
pub struct SourceRef {
    #[serde(rename(serialize = "sourcePath"))]
    pub(crate) source_path: String,
    #[serde(rename(serialize = "lineNumber"))]
    pub line_no: usize,
    pub(crate) column: usize,
    pub(crate) name: String,
    pub(crate) text: String,
    #[serde(skip_serializing)]
    pub(crate) matcher: Regex,
    pub(crate) vars: Vec<String>,
}

impl SourceRef {
    pub(crate) fn new(code: &CodeSource, result: QueryResult) -> SourceRef {
        let range = result.range;
        let source = code.buffer.as_str();
        let text = source[range.start_byte..range.end_byte].to_string();
        let line = range.start_point.row + 1;
        let col = range.start_point.column;
        let start = range.start_byte + 1;
        let mut end = range.end_byte - 1;
        if start == range.end_byte {
            end = range.end_byte;
        }
        let unquoted = &source[start..end].to_string();
        // println!("{} line {}", code.filename, line);
        let matcher = build_matcher(unquoted);
        let vars = Vec::new();
        let name = source[result.name_range].to_string();
        SourceRef {
            source_path: code.filename.clone(),
            line_no: line,
            column: col,
            name,
            text,
            matcher,
            vars,
        }
    }

    pub fn captures<'a>(&self, log_ref: &LogRef<'a>) -> Option<Captures<'a>> {
        self.matcher.captures(log_ref.line)
    }
}

impl fmt::Display for SourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[Line: {}, Col: {}] source `{}` name `{}` vars={:?}",
            self.line_no, self.column, self.text, self.name, self.vars
        )
    }
}

impl PartialEq for SourceRef {
    fn eq(&self, other: &Self) -> bool {
        self.line_no == other.line_no
            && self.column == other.column
            && self.name == other.name
            && self.text == other.text
            && self.vars == other.vars
    }
}

fn build_matcher(text: &str) -> Regex {
    // XXX: avoid regex that are too greedy by returning a regex that
    //      never matches anything
    if text == "{}" || text.trim() == "" {
        Regex::new(r#"\w\b\w"#).unwrap()
    } else {
        let curly_replacer = Regex::new(r#"\\?\{.*?\}"#).unwrap();
        let escaped = curly_replacer
            .split(text)
            .map(|s| regex::escape(s))
            .collect::<Vec<String>>()
            .join(r#"(\w+)"#);
        // println!("escaped = {}", Regex::new(&escaped).unwrap().as_str());
        Regex::new(&escaped).unwrap()
    }
}

#[test]
fn test_build_matcher_needs_escape() {
    let matcher = build_matcher("{}) {}, {}");
    assert_eq!(
        Regex::new(r#"(\w+)\) (\w+), (\w+)"#).unwrap().as_str(),
        matcher.as_str()
    );
}

#[test]
fn test_build_matcher_mix() {
    let matcher = build_matcher("{}) {:?}, {foo.bar}");
    assert_eq!(
        Regex::new(r#"(\w+)\) (\w+), (\w+)"#).unwrap().as_str(),
        matcher.as_str()
    );
}
