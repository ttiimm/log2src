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
    pub source_path: String,
    pub language: SourceLanguage,
    #[serde(rename(serialize = "lineNumber"))]
    pub line_no: usize,
    #[serde(rename(serialize = "endLineNumber"))]
    pub end_line_no: usize,
    pub column: usize,
    pub name: String,
    pub text: String,
    pub quality: usize,
    #[serde(skip_serializing)]
    pub(crate) matcher: Regex,
    pub pattern: String,
    pub(crate) args: Vec<FormatArgument>,
    pub(crate) vars: Vec<String>,
}

struct MessageMatcher {
    matcher: Regex,
    quality: usize,
    pattern: String,
    args: Vec<FormatArgument>,
}

impl SourceRef {
    pub(crate) fn new(code: &CodeSource, result: QueryResult) -> Option<SourceRef> {
        let range = result.range;
        let source = code.buffer.as_str();
        let text = source[range.start_byte..range.end_byte].to_string();
        let line_no = range.start_point.row + 1;
        let end_line_no = range.end_point.row + 1;
        let col = range.start_point.column;
        let start = range.start_byte + 1;
        let mut end = range.end_byte - 1;
        if start == range.end_byte {
            end = range.end_byte;
        }
        let unquoted = &source[start..end].to_string();
        // println!("{} line {}", code.filename, line);
        if let Some(MessageMatcher {
            matcher,
            pattern,
            args,
            quality,
        }) = build_matcher(unquoted, code.info.language)
        {
            let name = source[result.name_range].to_string();
            Some(SourceRef {
                source_path: code.filename.clone(),
                language: code.info.language,
                line_no,
                end_line_no,
                column: col,
                name,
                text,
                quality,
                matcher,
                pattern,
                args,
                vars: vec![],
            })
        } else {
            None
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
    LazyLock::new(|| Regex::new(r#"\{.*}|\\\{(.*)}"#).unwrap());

static CPP_PLACEHOLDER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"%[-+ #0]*\d*(?:\.\d+)?[hlLzjt]*[diuoxXfFeEgGaAcspn%]|\{(?:([a-zA-Z_][a-zA-Z0-9_.]*)|(\d+))?\s*(?::[^}]*)?}"#).unwrap()
});

fn placeholder_regex_for(language: SourceLanguage) -> &'static Regex {
    match language {
        SourceLanguage::Rust => RUST_PLACEHOLDER_REGEX.deref(),
        SourceLanguage::Java => JAVA_PLACEHOLDER_REGEX.deref(),
        SourceLanguage::Cpp => CPP_PLACEHOLDER_REGEX.deref(),
    }
}

fn build_matcher(text: &str, language: SourceLanguage) -> Option<MessageMatcher> {
    let mut args = Vec::new();
    let mut last_end = 0;
    let mut pattern = "(?s)^".to_string();
    let mut quality = 0;
    for cap in placeholder_regex_for(language).captures_iter(text) {
        let placeholder = cap.get(0).unwrap();
        let text = escape_ignore_newlines(&text[last_end..placeholder.start()]);
        quality += text.chars().filter(|c| !c.is_whitespace()).count();
        pattern.push_str(text.as_str());
        last_end = placeholder.end();
        pattern.push_str("(.+)");
        args.push(match (cap.get(1), cap.get(2)) {
            (Some(expr), None) => FormatArgument::Named(expr.as_str().to_string()),
            (None, Some(pos)) => FormatArgument::Positional(pos.as_str().parse().unwrap_or(0)),
            (Some(_), Some(_)) => unreachable!(),
            (None, None) => FormatArgument::Placeholder,
        });
    }
    let text = escape_ignore_newlines(&text[last_end..]);
    quality += text.chars().filter(|c| !c.is_whitespace()).count();
    if quality == 0 {
        None
    } else {
        pattern.push_str(text.as_str());
        pattern.push('$');
        Some(MessageMatcher {
            matcher: Regex::new(pattern.as_str()).unwrap(),
            quality,
            pattern,
            args,
        })
    }
}

/// Escape special chars except newlines and carriage returns in order to support multiline strings
fn escape_ignore_newlines(segment: &str) -> String {
    let mut result = String::with_capacity(segment.len() * 2);
    for c in segment.chars() {
        match c {
            '\n' => result.push_str(r"\n"), // Use actual newline in regex
            '\r' => result.push_str(r"\r"), // Handle carriage returns too
            // Escape regex special chars
            '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_matcher_needs_escape() {
        let MessageMatcher {
            matcher,
            pattern: _pat,
            args: _args,
            ..
        } = build_matcher("{}) {}, {}", SourceLanguage::Rust).unwrap();
        assert_eq!(
            Regex::new(r#"(?s)^(.+)\) (.+), (.+)$"#).unwrap().as_str(),
            matcher.as_str()
        );
    }

    #[test]
    fn test_build_matcher_named() {
        let MessageMatcher { matcher, .. } =
            build_matcher("abc {main_path:?} def", SourceLanguage::Rust).unwrap();
        assert_eq!(
            Regex::new(r#"(?s)^abc (.+) def$"#).unwrap().as_str(),
            matcher.as_str()
        );
    }

    #[test]
    fn test_build_matcher_mix() {
        let MessageMatcher { matcher, args, .. } =
            build_matcher("{}) {:?}, {foo.bar}", SourceLanguage::Rust).unwrap();
        assert_eq!(
            Regex::new(r#"(?s)^(.+)\) (.+), (.+)$"#).unwrap().as_str(),
            matcher.as_str()
        );
        assert_eq!(args[2], FormatArgument::Named("foo.bar".to_string()));
    }

    #[test]
    fn test_build_matcher_positional() {
        let MessageMatcher { matcher, args, .. } =
            build_matcher("second={2}", SourceLanguage::Rust).unwrap();
        assert_eq!(
            Regex::new(r#"(?s)^second=(.+)$"#).unwrap().as_str(),
            matcher.as_str()
        );
        assert_eq!(args[0], FormatArgument::Positional(2));
    }

    #[test]
    fn test_build_matcher_cpp() {
        let MessageMatcher { matcher, args, .. } =
            build_matcher("they are %d years old", SourceLanguage::Cpp).unwrap();
        assert_eq!(
            Regex::new(r#"(?s)^they are (.+) years old$"#)
                .unwrap()
                .as_str(),
            matcher.as_str()
        );
        assert_eq!(args[0], FormatArgument::Placeholder);
    }

    #[test]
    fn test_build_matcher_cpp_spdlog() {
        let MessageMatcher { matcher, args, .. } =
            build_matcher("they are {0:d} years old", SourceLanguage::Cpp).unwrap();
        assert_eq!(
            Regex::new(r#"(?s)^they are (.+) years old$"#)
                .unwrap()
                .as_str(),
            matcher.as_str()
        );
        assert_eq!(args[0], FormatArgument::Positional(0));
    }

    #[test]
    fn test_build_matcher_none() {
        let build_res = build_matcher("%s", SourceLanguage::Cpp);
        assert!(build_res.is_none());
    }

    #[test]
    fn test_build_matcher_multiline() {
        let MessageMatcher { matcher, .. } = build_matcher(
            "you're only as funky\n as your last cut",
            SourceLanguage::Rust,
        )
        .unwrap();
        assert_eq!(
            Regex::new(r#"(?s)^you're only as funky\n as your last cut$"#)
                .unwrap()
                .as_str(),
            matcher.as_str()
        );
    }
}
