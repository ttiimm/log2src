use regex::{Captures, Regex};

use crate::LogRef;

#[derive(Clone, Debug)]
pub struct LogFormat {
    regex: Regex,
}

impl LogFormat {
    pub fn new(format: Option<String>) -> Option<LogFormat> {
        format.map(|fmt| LogFormat {
            // TODO handle more gracefully if wrong format
            regex: Regex::new(&fmt).unwrap(),
        })
    }

    pub fn has_hints(self: LogFormat) -> bool {
        let mut flatten = self.regex.capture_names().flatten();
        flatten.any(|name| name == "line") && flatten.any(|name| name == "file")
    }

    pub fn build_src_filter(&self, log_refs: &Vec<LogRef>) -> Vec<String> {
        let mut results = Vec::new();
        for log_ref in log_refs {
            let captures = self.captures(log_ref);
            if let Some(file_match) = captures.name("file") {
                results.push(file_match.as_str().to_string());
            }
        }
        results
    }

    pub fn captures<'a>(&self, log_ref: &LogRef<'a>) -> Captures<'a> {
        self.regex
            .captures(log_ref.line)
            .unwrap_or_else(|| panic!("Couldn't match `{}` with `{:?}`", log_ref.line, self.regex))
    }
}

// TODO finish these tests
#[test]
fn test_has_line_support() {}
