use regex::{Captures, Regex};

use crate::LogRef;

#[derive(Clone, Debug)]
pub struct LogFormat {
    regex: Regex,
}

impl LogFormat {
    pub fn new(format: String) -> LogFormat {
        LogFormat {
            // TODO handle more gracefully if wrong format
            regex: Regex::new(&format).unwrap(),
        }
    }

    pub fn has_src_hint(self: LogFormat) -> bool {
        let mut flatten = self.regex.capture_names().flatten();
        flatten.any(|name| name == "line") && flatten.any(|name| name == "file")
    }

    pub fn build_src_filter(&self, log_refs: &Vec<LogRef>) -> Option<Vec<String>> {
        let mut results = Vec::new();
        for log_ref in log_refs {
            let captures = self.captures(log_ref.line);
            if let Some(file_match) = captures.name("file") {
                results.push(file_match.as_str().to_string());
            }
        }
        (!results.is_empty()).then_some(results)
    }

    pub fn captures<'a>(&self, line: &'a str) -> Captures<'a> {
        self.regex
            .captures(line)
            .unwrap_or_else(|| panic!("Couldn't match `{}` with `{:?}`", line, self.regex))
    }
}

// TODO finish these tests
#[test]
fn test_has_line_support() {}
