use regex::{Captures, Regex, RegexBuilder};

use crate::{LogError, LogRef};

#[derive(Clone, Debug)]
pub struct LogFormat {
    regex: Regex,
}

impl LogFormat {
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

impl TryFrom<&str> for LogFormat {
    type Error = LogError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        fn check_captures(regex: &Regex) -> Result<(), LogError> {
            let mut seen = Vec::new();
            for name in regex.capture_names().filter_map(|x| x) {
                match name {
                    "timestamp" | "thread" | "method" | "file" | "line" | "body" | "level" => {
                        seen.push(name)
                    }
                    _ => {
                        return Err(LogError::UnknownFormatCapture {
                            name: name.to_string(),
                        })
                    }
                }
            }
            if !seen.contains(&"body") {
                return Err(LogError::FormatMissingCapture {
                    name: "body".to_string(),
                });
            }
            Ok(())
        }

        RegexBuilder::new(&value)
            // XXX: This is kinda a hack to support multiline matching in lnav, but
            // not really useful for log2src atm because its still filtering line-by-line,
            // so this case would never come up
            .dot_matches_new_line(true)
            .build()
            .map_err(|source| LogError::InvalidFormatRegex { source })
            .and_then(|regex| {
                check_captures(&regex)?;
                Ok(LogFormat { regex })
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::LogFormat;
    use insta::assert_snapshot;
    use miette::{IntoDiagnostic, NarratableReportHandler, Report};

    fn get_pretty_report_string(error: Report) -> String {
        let mut buffer = String::new();
        let handler = NarratableReportHandler::new();
        let handler = if let Some(help) = error.help() {
            handler.with_footer(help.to_string())
        } else {
            handler
        };
        let _ = handler.render_report(&mut buffer, error.as_ref());
        buffer
    }

    #[test]
    fn test_invalid_regex() {
        let res = Report::from(LogFormat::try_from("abc(").unwrap_err());
        let rep = get_pretty_report_string(res);
        assert_snapshot!(rep);
    }

    #[test]
    fn test_no_body() {
        let res = LogFormat::try_from("abc").into_diagnostic();
        let rep = get_pretty_report_string(res.unwrap_err());
        assert_snapshot!(rep);
    }

    #[test]
    fn test_unknown_cap() {
        let res = LogFormat::try_from("abc(?<extra>def)").into_diagnostic();
        let rep = get_pretty_report_string(res.unwrap_err());
        assert_snapshot!(rep);
    }
}
