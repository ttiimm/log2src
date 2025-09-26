use clap::Parser as ClapParser;
use indicatif::{ProgressBar, ProgressStyle};
use log2src::{filter_log, LogError, LogMapping, LogMatcher, ProgressTracker, ProgressUpdate};
use miette::{IntoDiagnostic, Report};
use serde_json::{self};
use std::io::stdout;
use std::sync::atomic::Ordering;
use std::thread::sleep;
use std::time::Duration;
use std::{fs, io, path::PathBuf};

/// The log2src command maps log statements back to the source code that emitted them.
#[derive(ClapParser)]
#[command(author, version, about, long_about)]
struct Cli {
    /// A source directory (or soon directoires) to map logs onto
    #[arg(short = 'd', long, value_name = "SOURCES")]
    sources: String,

    /// A log file to use, if not from stdin
    #[arg(short, long, value_name = "LOG")]
    log: Option<PathBuf>,

    /// The regex of a log format being used
    #[arg(short, long, value_name = "FORMAT")]
    format: Option<String>,

    /// The line in the log to use (0 based)
    #[arg(short, long, value_name = "START")]
    start: Option<usize>,

    /// The last line of the log to use (0 based)
    #[arg(short, long, value_name = "END")]
    end: Option<usize>,

    /// Print progress information to standard error
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> miette::Result<()> {
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
                    }
                }
            }
        });
    }

    let format_re = if let Some(format) = args.format {
        Some(format.as_str().try_into()?)
    } else {
        None
    };

    let input = args.log;
    let mut reader: Box<dyn io::Read> = match input {
        None => Box::new(io::stdin()),
        Some(filename) => Box::new(fs::File::open(filename).expect("Can open file")),
    };

    let mut buffer = String::new();
    reader.read_to_string(&mut buffer).into_diagnostic()?;
    let filter = args.start.unwrap_or(0)..args.end.unwrap_or(usize::MAX);

    let filtered = filter_log(&buffer, filter, format_re);
    let mut log_matcher = LogMatcher::new();
    log_matcher
        .add_root(&PathBuf::from(args.sources))
        .into_diagnostic()?;
    log_matcher
        .discover_sources(&tracker)
        .into_iter()
        .for_each(|err| eprintln!("{:?}", Report::new(err)));
    log_matcher.extract_log_statements(&tracker);
    if log_matcher.is_empty() {
        return Err(LogError::NoLogStatements.into());
    }
    let log_mappings = filtered
        .iter()
        .flat_map(|log_ref| log_matcher.match_log_statement(log_ref))
        .collect::<Vec<LogMapping>>();

    for mapping in log_mappings {
        let serialized = serde_json::to_string(&mapping).unwrap();
        println!("{}", serialized);
    }

    Ok(())
}
