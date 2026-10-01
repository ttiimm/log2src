use clap::Parser as ClapParser;
use colored_json::{ColoredFormatter, CompactFormatter, Styler};
use indicatif::{ProgressBar, ProgressStyle};
use log2src::{
    set_tracker_once, Cache, LogError, LogFormat, LogMapping, LogMatcher, LogRef, LogRefBuilder,
    ProgressTracker, ProgressUpdate,
};
use miette::{IntoDiagnostic, MietteHandlerOpts, Report};
use serde::Serialize;
use std::collections::BTreeMap;
use std::io::{stdout, BufRead, BufReader};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use std::{env, fs, io, path::PathBuf};

fn get_footer() -> String {
    let mut footer = String::new();
    if let Ok(cache) = Cache::open() {
        footer.push_str("Paths:\n");
        footer.push_str(
            format!(
                "    Cache directory: {}\n",
                cache.location().to_string_lossy()
            )
            .as_str(),
        );
    }
    footer.push_str("\nFor more information, see https://github.com/ttiimm/log2src\n");
    footer
}

/// The log2src command maps log statements back to the source code that emitted them.
#[derive(ClapParser)]
#[command(author, version, about, long_about)]
#[command(after_help = get_footer())]
struct Cli {
    /// The source directories to map logs onto
    #[arg(short = 'd', long, value_name = "SOURCES")]
    sources: Vec<String>,

    /// A log file to use, if not from stdin
    #[arg(short, long, value_name = "LOG")]
    log: Option<PathBuf>,

    /// The regex of a log format being used
    #[arg(short, long, value_name = "FORMAT")]
    format: Option<String>,

    /// The first line in the log to use (0 based)
    #[arg(short, long, value_name = "START")]
    start: Option<usize>,

    /// The number of log messages to process
    #[arg(short, long, value_name = "COUNT")]
    count: Option<usize>,

    /// Aggregate matching messages by source statement
    #[arg(long)]
    summary: bool,

    /// Print progress information to standard error
    #[arg(short, long)]
    verbose: bool,
}

fn get_colored_formatter() -> ColoredFormatter<CompactFormatter> {
    let compact_formatter = CompactFormatter {};

    ColoredFormatter::with_styler(compact_formatter, Styler::default())
}

#[must_use]
struct MessageAccumulator {
    log_matcher: LogMatcher,
    log_format: Option<LogFormat>,
    content: String,
    message_count: usize,
    limit: usize,
    summary: bool,
    message_start_line: usize,
    source_usage: BTreeMap<(String, usize, usize, String), SourceUsage>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceUsage {
    source_path: String,
    line_number: usize,
    column: usize,
    name: String,
    count: usize,
    samples: Vec<LogSample>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogSample {
    line_number: usize,
    text: String,
}

impl MessageAccumulator {
    fn new(
        log_matcher: LogMatcher,
        log_format: Option<LogFormat>,
        limit: usize,
        summary: bool,
    ) -> Self {
        Self {
            log_matcher,
            log_format,
            content: String::new(),
            message_count: 0,
            limit,
            summary,
            message_start_line: 0,
            source_usage: BTreeMap::new(),
        }
    }

    fn get_log_mapping<'a>(&self, log_ref: LogRef<'a>) -> LogMapping<'a> {
        self.log_matcher
            .match_log_statement(&log_ref)
            .unwrap_or_else(move || LogMapping {
                log_ref,
                src_ref: None,
                variables: vec![],
                exception_trace: vec![],
            })
    }

    fn process_msg(&mut self) {
        if let Some(captures) = self.log_format.as_ref().unwrap().captures(&self.content) {
            self.message_count += 1;
            let log_ref = LogRefBuilder::new().build_from_captures(captures, &self.content);
            let log_mapping = self.get_log_mapping(log_ref);
            if self.summary {
                if let Some((source_path, source_line, column, name)) =
                    log_mapping.src_ref.map(|src_ref| {
                        (
                            src_ref.source_path.clone(),
                            src_ref.line_no,
                            src_ref.column,
                            src_ref.name.clone(),
                        )
                    })
                {
                    Self::record_source_usage(
                        &mut self.source_usage,
                        &source_path,
                        source_line,
                        column,
                        &name,
                        &self.content,
                        self.message_start_line,
                    );
                }
            } else {
                let serialized = get_colored_formatter().to_colored_json_auto(&log_mapping);
                println!("{}", serialized.unwrap());
            }
        }
        self.content.clear();
    }

    fn new_msg(&mut self, line: &str, line_number: usize) {
        if !self.content.is_empty() {
            self.process_msg();
        }

        self.message_start_line = line_number;
        self.content.push_str(line);
    }

    fn continued_line(&mut self, line: &str) {
        if self.content.is_empty() {
            return;
        }
        self.content.push('\n');
        self.content.push_str(line);
    }

    fn process_bare_msg(&mut self, line: &str, line_number: usize) {
        let log_ref = LogRefBuilder::new().with_body(Some(line)).build(line);
        let log_mapping = self.get_log_mapping(log_ref);
        if self.summary {
            if let Some((source_path, source_line, column, name)) =
                log_mapping.src_ref.map(|src_ref| {
                    (
                        src_ref.source_path.clone(),
                        src_ref.line_no,
                        src_ref.column,
                        src_ref.name.clone(),
                    )
                })
            {
                Self::record_source_usage(
                    &mut self.source_usage,
                    &source_path,
                    source_line,
                    column,
                    &name,
                    line,
                    line_number,
                );
            }
        } else {
            println!(
                "{}",
                get_colored_formatter()
                    .to_colored_json_auto(&log_mapping)
                    .unwrap()
            );
        }
    }

    fn record_source_usage(
        source_usage: &mut BTreeMap<(String, usize, usize, String), SourceUsage>,
        source_path: &str,
        source_line: usize,
        column: usize,
        name: &str,
        message: &str,
        log_line: usize,
    ) {
        let key = (
            source_path.to_string(),
            source_line,
            column,
            name.to_string(),
        );
        let usage = source_usage.entry(key).or_insert_with(|| SourceUsage {
            source_path: source_path.to_string(),
            line_number: source_line,
            column,
            name: name.to_string(),
            count: 0,
            samples: Vec::new(),
        });
        usage.count += 1;
        if usage.samples.len() < 3 {
            usage.samples.push(LogSample {
                line_number: log_line + 1,
                text: message.chars().take(240).collect(),
            });
        }
    }

    fn consume_line(&mut self, line: &str, line_number: usize) {
        match &self.log_format {
            Some(format) => {
                if format.is_match(line) {
                    self.new_msg(line, line_number);
                } else {
                    self.continued_line(line);
                }
            }
            None => {
                self.process_bare_msg(line, line_number);
            }
        }
    }

    fn flush(&mut self) {
        if !self.content.is_empty() && !self.at_limit() {
            self.process_msg();
        }
    }

    fn at_limit(&self) -> bool {
        self.message_count >= self.limit
    }

    fn eof(mut self) -> miette::Result<()> {
        self.flush();

        if self.summary {
            let usages = self.source_usage.values().collect::<Vec<_>>();
            println!(
                "{}",
                get_colored_formatter()
                    .to_colored_json_auto(&usages)
                    .unwrap()
            );
            return Ok(());
        }

        if self.log_format.is_some() && self.message_count == 0 {
            Err(LogError::NoLogMessages.into())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Serialize)]
struct SerializableDiagnostic {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    severity: Option<miette::Severity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    help: Option<String>,
}

impl From<Report> for SerializableDiagnostic {
    fn from(value: Report) -> Self {
        Self {
            message: value.to_string(),
            code: value.code().map(|c| c.to_string()),
            severity: value.severity(),
            source: value.source().map(|s| s.to_string()),
            help: value.help().map(|h| h.to_string()),
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorWrapper {
    error: SerializableDiagnostic,
}

fn main() -> miette::Result<()> {
    let _ = miette::set_hook(Box::new(move |_| {
        Box::new(
            MietteHandlerOpts::new()
                .width(env::var("COLS").unwrap_or_default().parse().unwrap_or(80))
                .break_words(false)
                .build(),
        )
    }));

    let args = Cli::parse();
    let mut tracker = ProgressTracker::new();
    let listener = if args.verbose {
        Some(tracker.subscribe())
    } else {
        None
    };
    let tracker = Arc::new(tracker);
    set_tracker_once(Arc::clone(&tracker));

    if let Some(listener) = listener {
        std::thread::spawn(move || {
            let mut prefix = String::new();
            for update in listener {
                match update {
                    ProgressUpdate::Step(msg) => eprintln!("{}", msg),
                    ProgressUpdate::BeginStep(msg) => {
                        prefix = msg;
                    }
                    ProgressUpdate::EndStep(msg) => {
                        eprintln!("{}... {}", prefix, msg);
                        prefix.clear();
                    }
                    ProgressUpdate::Work(info) => {
                        // XXX Take the stdout lock so that the actual output does not interfere
                        // with the progress bar updates on stderr.
                        let _stdout_lock = stdout().lock();
                        let bar = ProgressBar::new(info.total)
                            .with_prefix(prefix.clone())
                            .with_style(
                                ProgressStyle::with_template("{prefix}... {bar} {pos:>7}/{len:7}")
                                    .unwrap(),
                            );
                        while info.is_in_progress() {
                            bar.set_position(info.completed.load(Ordering::Relaxed));
                            sleep(Duration::from_millis(33));
                        }
                        bar.finish_and_clear();
                    }
                }
            }
        });
    }

    let log_format: Option<LogFormat> = if let Some(format) = args.format {
        Some(format.as_str().try_into()?)
    } else {
        None
    };

    let reader: Box<dyn io::Read> = match args.log {
        None => Box::new(io::stdin()),
        Some(filename) => {
            let path = filename;
            match fs::File::open(&path) {
                Ok(file) => Box::new(file),
                Err(err) => {
                    return Err(LogError::CannotReadLogFile {
                        path,
                        source: err.into(),
                    }
                    .into());
                }
            }
        }
    };

    let mut log_matcher = LogMatcher::new();
    for source in &args.sources {
        log_matcher
            .add_root(&PathBuf::from(source))
            .into_diagnostic()?;
    }

    let cache_open_res = Cache::open();
    if args.verbose && cache_open_res.is_err() {
        eprintln!("Could not find cache directory, will not cache source trees");
    }

    if let Ok(cache) = &cache_open_res {
        let res = log_matcher.load_from_cache(cache);
        for err in res {
            let report = Report::new(err);
            if args.verbose
                || report.severity().unwrap_or(miette::Severity::Error) != miette::Severity::Advice
            {
                eprintln!("{:?}", report);
            }
        }
    }

    log_matcher
        .discover_sources()
        .into_iter()
        .for_each(|err| eprintln!("{:?}", Report::new(err)));
    let result = log_matcher.extract_log_statements();
    if log_matcher.is_empty() {
        return Err(LogError::NoLogStatements.into());
    }

    if !result.errors.is_empty() {
        for err in result.errors {
            eprintln!("{:?}", Report::new(err));
        }
    }

    if result.summary.changes() > 0 {
        if let Ok(cache) = &cache_open_res {
            let res = log_matcher.cache_to(cache);
            if let Err(err) = res {
                eprintln!("{:?}", Report::new(err));
            }
        }
    }

    let start = args.start.unwrap_or(0);
    let count = args.count.unwrap_or(usize::MAX);
    let mut accumulator = MessageAccumulator::new(log_matcher, log_format, count, args.summary);

    let reader = BufReader::new(reader);
    for (lineno, line_res) in reader.lines().skip(start).enumerate() {
        if accumulator.at_limit() {
            break;
        }
        match line_res {
            Ok(line) => accumulator.consume_line(&line, start + lineno),
            Err(err) => {
                accumulator.flush();
                let report: Report = LogError::UnableToReadLine {
                    line: lineno,
                    source: err.into(),
                }
                .into();
                let wrapper = ErrorWrapper {
                    error: SerializableDiagnostic::from(report),
                };
                let serialized = get_colored_formatter()
                    .to_colored_json_auto(&wrapper)
                    .unwrap();
                if args.summary {
                    eprintln!("{}", serialized);
                } else {
                    println!("{}", serialized);
                }
            }
        }
    }

    accumulator.eof()
}

#[cfg(test)]
mod tests {
    use super::MessageAccumulator;
    use std::collections::BTreeMap;

    #[test]
    fn source_usage_counts_matches_and_keeps_only_three_samples() {
        let mut source_usage = BTreeMap::new();
        for line_number in 0..5 {
            MessageAccumulator::record_source_usage(
                &mut source_usage,
                "src/main.rs",
                12,
                4,
                "log::info",
                &format!("request {line_number}"),
                line_number,
            );
        }

        let usage = source_usage.values().next().unwrap();
        assert_eq!(usage.count, 5);
        assert_eq!(usage.samples.len(), 3);
        assert_eq!(usage.samples[0].line_number, 1);
        assert_eq!(usage.samples[2].text, "request 2");
    }
}
