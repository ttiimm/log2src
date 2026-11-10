use log2src::{LogFormat, LogMatcher, LogRefBuilder};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

mod common;

#[derive(Debug)]
struct Label {
    file: String,
    line: usize,
}

#[derive(Debug)]
struct Input {
    nohint: String,
    label: Option<Label>,
}

#[derive(Default, Debug)]
struct Report {
    /// Input lines processed, including excluded lines.
    total: usize,
    /// Lines that did not match the hinted log format.
    parse_failures: usize,
    /// Parsed lines whose source hint is not a Java filename.
    unsupported: usize,
    /// Parsed Java lines whose hinted filename is absent from the source root.
    absent: usize,
    /// Parsed Java lines whose hinted filename occurs in multiple source files.
    ambiguous: usize,
    /// Lines with a uniquely resolved Java source hint, used as the agreement denominator.
    evaluated: usize,
    /// Evaluated lines for which the matcher returned no source reference.
    unmatched: usize,
    /// Evaluated lines mapped to a file other than the uniquely resolved hinted file.
    wrong_file: usize,
    /// Evaluated lines mapped to the hinted file, regardless of line agreement.
    correct_file: usize,
    /// Correct-file lines whose hinted line falls within the extracted source range.
    correct_line: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Unmatched,
    WrongFile,
    WrongLine,
    CorrectLine,
}

fn generate_inputs(
    input: impl BufRead,
    mut output: impl Write,
    max_lines: Option<usize>,
) -> Result<Vec<Input>, Box<dyn Error>> {
    let format = LogFormat::try_from(common::FORMAT_HINT)?;
    let nohint_format = LogFormat::try_from(common::FORMAT_NOHINT)?;
    let mut inputs = Vec::new();
    for (index, line) in input
        .lines()
        .take(max_lines.unwrap_or(usize::MAX))
        .enumerate()
    {
        let line = line?;
        let (nohint, label) = if let Some(captures) = format.captures(&line) {
            let file = captures.name("file").ok_or("missing file capture")?;
            let line_number = captures.name("line").ok_or("missing line capture")?;
            let label = Label {
                file: file.as_str().to_owned(),
                line: line_number.as_str().parse()?,
            };
            if label.line == 0 {
                return Err(format!("invalid source line on input line {}", index + 1).into());
            }
            let nohint = format!(
                "{}nohint{}",
                &line[..file.start()],
                &line[line_number.end()..]
            );
            let nohint_captures = nohint_format
                .captures(&nohint)
                .ok_or_else(|| format!("generated invalid no-hint line {}", index + 1))?;
            if captures.name("body").map(|m| m.as_str())
                != nohint_captures.name("body").map(|m| m.as_str())
            {
                return Err(format!("body changed on input line {}", index + 1).into());
            }
            (nohint, Some(label))
        } else {
            (line, None)
        };
        writeln!(output, "{nohint}")?;
        inputs.push(Input { nohint, label });
    }
    output.flush()?;
    Ok(inputs)
}

fn java_files(root: &Path) -> Result<HashMap<String, Vec<PathBuf>>, Box<dyn Error>> {
    let mut files: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for entry in WalkDir::new(root) {
        let entry = entry?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "java")
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            files.entry(name).or_default().push(entry.into_path());
        }
    }
    Ok(files)
}

fn evaluate(
    inputs: &[Input],
    files: &HashMap<String, Vec<PathBuf>>,
    matcher: &LogMatcher,
) -> Result<(Report, Vec<Option<Outcome>>), Box<dyn Error>> {
    let format = LogFormat::try_from(common::FORMAT_NOHINT)?;
    let mut report = Report::default();
    let mut outcomes = Vec::with_capacity(inputs.len());
    for input in inputs {
        report.total += 1;
        let Some(label) = &input.label else {
            report.parse_failures += 1;
            outcomes.push(None);
            continue;
        };
        if !label.file.ends_with(".java") {
            report.unsupported += 1;
            outcomes.push(None);
            continue;
        }
        let Some(paths) = files.get(&label.file) else {
            report.absent += 1;
            outcomes.push(None);
            continue;
        };
        let [expected_path] = paths.as_slice() else {
            report.ambiguous += 1;
            outcomes.push(None);
            continue;
        };
        report.evaluated += 1;
        let captures = format
            .captures(&input.nohint)
            .ok_or("generated no-hint input no longer parses")?;
        let log_ref = LogRefBuilder::new().build_from_captures(captures, &input.nohint);
        let outcome = match matcher
            .match_log_statement(&log_ref)
            .and_then(|m| m.src_ref)
        {
            None => {
                report.unmatched += 1;
                Outcome::Unmatched
            }
            Some(source) if Path::new(&source.source_path) != expected_path => {
                report.wrong_file += 1;
                Outcome::WrongFile
            }
            Some(source) => {
                report.correct_file += 1;
                if (source.line_no..=source.end_line_no).contains(&label.line) {
                    report.correct_line += 1;
                    Outcome::CorrectLine
                } else {
                    Outcome::WrongLine
                }
            }
        };
        outcomes.push(Some(outcome));
    }
    Ok((report, outcomes))
}

fn print_report(strategy: &str, report: &Report) {
    println!("\n{strategy}");
    println!("  total lines: {}", report.total);
    println!("  parse failures: {}", report.parse_failures);
    println!("  non-Java labels: {}", report.unsupported);
    println!("  labels absent from source root: {}", report.absent);
    println!("  ambiguous filenames: {}", report.ambiguous);
    println!("  evaluated: {}", report.evaluated);
    println!("  unmatched: {}", report.unmatched);
    println!("  wrong file: {}", report.wrong_file);
    println!("  correct file: {}", report.correct_file);
    println!(
        "  correct file and line in extracted range: {}",
        report.correct_line
    );
    if report.evaluated > 0 {
        println!(
            "  file agreement: {:.2}%",
            100.0 * report.correct_file as f64 / report.evaluated as f64
        );
        println!(
            "  file and line-range agreement: {:.2}%",
            100.0 * report.correct_line as f64 / report.evaluated as f64
        );
    }
}

fn print_comparison(baseline: &[Option<Outcome>], aho: &[Option<Outcome>]) {
    let mut both_correct = 0;
    let mut baseline_only = 0;
    let mut aho_only = 0;
    let mut neither = 0;
    for (baseline, aho) in baseline.iter().zip(aho) {
        if let (Some(baseline), Some(aho)) = (baseline, aho) {
            match (
                *baseline == Outcome::CorrectLine,
                *aho == Outcome::CorrectLine,
            ) {
                (true, true) => both_correct += 1,
                (true, false) => baseline_only += 1,
                (false, true) => aho_only += 1,
                (false, false) => neither += 1,
            }
        }
    }
    println!("\nPer-line file and line-range agreement");
    println!("  both agree with label: {both_correct}");
    println!("  only per_file agrees: {baseline_only}");
    println!("  only aho agrees: {aho_only}");
    println!("  neither agrees: {neither}");
}

fn main() -> Result<(), Box<dyn Error>> {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--bench" | "--test" => {}
            filter if !filter.starts_with('-') => {
                if !filter.is_empty() && !("kafka_accuracy").contains(filter) {
                    eprintln!(
                        "skipping kafka_accuracy: benchmark filter {filter:?} does not match"
                    );
                    return Ok(());
                }
            }
            _ => return Err(format!("unsupported kafka_accuracy argument: {arg}").into()),
        }
    }
    let bench_data = std::env::var_os("BENCH_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bench-data"));
    let configured_root = std::env::var_os("LOG2SRC_BENCH_SRC");
    let root = configured_root
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| bench_data.join("kafka-src/clients/src/main/java"));
    let input_path = common::accuracy_log_path();
    let max_lines = common::accuracy_sample_lines()?;
    if !root.is_dir() {
        if configured_root.is_some() {
            return Err(format!("source root is not a directory: {}", root.display()).into());
        }
        eprintln!("skipping kafka_accuracy: {} missing", root.display());
        return Ok(());
    }
    if !input_path.is_file() {
        if std::env::var_os("LOG2SRC_ACCURACY_LOG").is_some() {
            return Err(format!("accuracy log is not a file: {}", input_path.display()).into());
        }
        eprintln!("skipping kafka_accuracy: {} missing", input_path.display());
        return Ok(());
    }
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(format!("source root is not a directory: {}", root.display()).into());
    }
    let nohint = tempfile::NamedTempFile::new()?;
    let inputs = generate_inputs(
        BufReader::new(File::open(&input_path)?),
        BufWriter::new(nohint.as_file()),
        max_lines,
    )?;
    let files = java_files(&root)?;
    let mut matcher = LogMatcher::new();
    matcher.add_root(&root)?;
    let errors = matcher.discover_sources();
    if !errors.is_empty() {
        for error in &errors {
            eprintln!("{error}");
        }
        return Err("source discovery failed".into());
    }
    let extraction = matcher.extract_log_statements();
    if !extraction.errors.is_empty() {
        for error in &extraction.errors {
            eprintln!("{error}");
        }
        return Err("source extraction failed".into());
    }
    if matcher.is_empty() {
        return Err("no source statements extracted".into());
    }
    println!("Source root: {}", root.display());
    println!("Hint log: {}", input_path.display());
    let (baseline, baseline_outcomes) = evaluate(&inputs, &files, &matcher)?;
    print_report("per_file", &baseline);
    matcher.build_aho_matchers();
    let (aho, aho_outcomes) = evaluate(&inputs, &files, &matcher)?;
    print_report("aho", &aho);
    print_comparison(&baseline_outcomes, &aho_outcomes);
    if baseline.evaluated == 0 {
        return Err("no uniquely labeled Java lines in source root".into());
    }
    Ok(())
}
