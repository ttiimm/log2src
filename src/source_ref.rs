use crate::{CodeSource, QueryResult, SourceLanguage};
use core::fmt;
use regex::{Captures, Regex};
use serde::Serialize;
use std::ops::Deref;
use std::sync::LazyLock;

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub enum FormatArgument {
    Named(String),
    Positional(usize),
    Placeholder,
}

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
    pub(crate) args: Vec<FormatArgument>,
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
        let (matcher, args) = build_matcher(unquoted, code.language);
        let name = source[result.name_range].to_string();
        SourceRef {
            source_path: code.filename.clone(),
            line_no: line,
            column: col,
            name,
            text,
            matcher,
            args,
            vars: vec![],
        }
    }

    pub fn captures<'a>(&self, line: &'a str) -> Option<Captures<'a>> {
        self.matcher.captures(line)
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

static RUST_PLACEHOLDER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\{(?:([a-zA-Z_][a-zA-Z0-9_.]*)|(\d+))?\s*(?::[^}]*)?}"#).unwrap()
});

static JAVA_PLACEHOLDER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\\?\{.*}"#).unwrap());

static CPP_PLACEHOLDER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"%[-+ #0]*\d*(\.\d+)?[hlLzjt]*[diuoxXfFeEgGaAcspn%]"#).unwrap());

fn placeholder_regex_for(language: SourceLanguage) -> &'static Regex {
    match language {
        SourceLanguage::Rust => RUST_PLACEHOLDER_REGEX.deref(),
        SourceLanguage::Java => JAVA_PLACEHOLDER_REGEX.deref(),
        SourceLanguage::Cpp => CPP_PLACEHOLDER_REGEX.deref(),
    }
}

fn build_matcher(text: &str, language: SourceLanguage) -> (Regex, Vec<FormatArgument>) {
    // XXX: avoid regex that are too greedy by returning a regex that
    //      never matches anything
    let mut args = Vec::new();
    if text == "{}" || text.trim() == "" {
        (Regex::new(r#"\w\b\w"#).unwrap(), args)
    } else {
        let mut last_end = 0;
        let mut pattern = "^".to_string();
        for cap in placeholder_regex_for(language).captures_iter(text) {
            let placeholder = cap.get(0).unwrap();
            pattern.push_str(regex::escape(&text[last_end..placeholder.start()]).as_str());
            last_end = placeholder.end();
            pattern.push_str("(.+)");
            args.push(match (cap.get(1), cap.get(2)) {
                (Some(expr), None) => FormatArgument::Named(expr.as_str().to_string()),
                (None, Some(pos)) => FormatArgument::Positional(pos.as_str().parse().unwrap_or(0)),
                (Some(_), Some(_)) => unreachable!(),
                (None, None) => FormatArgument::Placeholder,
            });
        }
        pattern.push_str(regex::escape(&text[last_end..]).as_str());
        pattern.push('$');
        (Regex::new(pattern.as_str()).unwrap(), args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_matcher_needs_escape() {
        let (matcher, _args) = build_matcher("{}) {}, {}", SourceLanguage::Rust);
        assert_eq!(
            Regex::new(r#"^(.+)\) (.+), (.+)$"#).unwrap().as_str(),
            matcher.as_str()
        );
    }

    #[test]
    fn test_build_matcher_named() {
        let (matcher, _args) = build_matcher("abc {main_path:?} def", SourceLanguage::Rust);
        assert_eq!(
            Regex::new(r#"^abc (.+) def$"#).unwrap().as_str(),
            matcher.as_str()
        );
    }

    #[test]
    fn test_build_matcher_mix() {
        let (matcher, args) = build_matcher("{}) {:?}, {foo.bar}", SourceLanguage::Rust);
        assert_eq!(
            Regex::new(r#"^(.+)\) (.+), (.+)$"#).unwrap().as_str(),
            matcher.as_str()
        );
        assert_eq!(args[2], FormatArgument::Named("foo.bar".to_string()));
    }

    #[test]
    fn test_build_matcher_positional() {
        let (matcher, args) = build_matcher("{2}", SourceLanguage::Rust);
        assert_eq!(Regex::new(r#"^(.+)$"#).unwrap().as_str(), matcher.as_str());
        assert_eq!(args[0], FormatArgument::Positional(2));
    }

    #[test]
    fn test_build_matcher_cpp() {
        let (matcher, args) = build_matcher("they are %d years old", SourceLanguage::Cpp);
        assert_eq!(Regex::new(r#"^they are (.+) years old$"#).unwrap().as_str(), matcher.as_str());
        assert_eq!(args[0], FormatArgument::Placeholder);
    }
}
