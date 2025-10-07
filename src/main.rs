use clap::Parser as ClapParser;
use colored_json::{ColoredFormatter, CompactFormatter, Styler};
use indicatif::{ProgressBar, ProgressStyle};
use log2src::{
    Cache, LogError, LogFormat, LogMapping, LogMatcher, LogRef, LogRefBuilder, ProgressTracker,
    ProgressUpdate,
};
use miette::{IntoDiagnostic, MietteHandlerOpts, Report};
use serde::Serialize;
use std::io::{stdout, BufRead, BufReader};
use std::sync::atomic::Ordering;
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
                cache.location.to_string_lossy()
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
}

impl MessageAccumulator {
    fn new(log_matcher: LogMatcher, log_format: Option<LogFormat>, limit: usize) -> Self {
        Self {
            log_matcher,
            log_format,
            content: String::new(),
            message_count: 0,
            limit,
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
            let serialized = get_colored_formatter().to_colored_json_auto(&log_mapping);
            println!("{}", serialized.unwrap());
        }
        self.content.clear();
    }

    fn new_msg(&mut self, line: &str) {
        if !self.content.is_empty() {
            self.process_msg();
        }

        self.content.push_str(line);
    }

    fn continued_line(&mut self, line: &str) {
        if self.content.is_empty() {
            return;
        }
        self.content.push('\n');
        self.content.push_str(line);
    }

    fn process_bare_msg(&self, line: &str) {
        let log_ref = LogRefBuilder::new().with_body(Some(line)).build(line);
        let log_mapping = self.get_log_mapping(log_ref);
        println!(
            "{}",
            get_colored_formatter()
                .to_colored_json_auto(&log_mapping)
                .unwrap()
        );
    }

    fn consume_line(&mut self, line: &str) {
        match &self.log_format {
            Some(format) => {
                if format.is_match(&line) {
                    self.new_msg(&line);
                } else {
                    self.continued_line(&line);
                }
            }
            None => {
                self.process_bare_msg(&line);
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
    let mut tracker = ProgressTracker::new();

    let args = Cli::parse();

    if args.verbose {
        let listener = tracker.subscribe();
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
            let path = PathBuf::from(filename);
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

    if let Ok(cache) = &cache_open_res {
        let res = log_matcher.load_from_cache(&cache, &tracker);
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
        .discover_sources(&tracker)
        .into_iter()
        .for_each(|err| eprintln!("{:?}", Report::new(err)));
    let extract_summary = log_matcher.extract_log_statements(&tracker);
    if log_matcher.is_empty() {
        return Err(LogError::NoLogStatements.into());
    }

    if extract_summary.changes() > 0 {
        if let Ok(cache) = &cache_open_res {
            let res = log_matcher.cache_to(&cache, &tracker);
            if let Err(err) = res {
                eprintln!("{:?}", Report::new(err));
            }
        }
    }

    let start = args.start.unwrap_or(0);
    let count = args.count.unwrap_or(usize::MAX);
    let mut accumulator = MessageAccumulator::new(log_matcher, log_format, count);

    let reader = BufReader::new(reader);
    for (lineno, line_res) in reader.lines().skip(start).enumerate() {
        if accumulator.at_limit() {
            break;
        }
        match line_res {
            Ok(line) => accumulator.consume_line(&line),
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
                println!(
                    "{}",
                    get_colored_formatter()
                        .to_colored_json_auto(&wrapper)
                        .unwrap()
                );
            }
        }
    }

    accumulator.eof()
}
