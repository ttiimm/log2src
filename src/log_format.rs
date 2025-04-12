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

    pub fn has_line_support(self: LogFormat) -> bool {
        self.regex
            .capture_names()
            .flatten()
            .any(|name| name == "line")
    }

    pub fn captures<'a>(&self, log_ref: &LogRef<'a>) -> Option<Captures<'a>> {
        self.regex.captures(log_ref.line)
    }
}

// TODO finish these tests
#[test]
fn test_has_line_support() {}
