use assert_cmd::prelude::*;
use std::{path::Path, process::Command};

#[test]
fn basic() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("log2src")?;
    let basic_source = Path::new("tests").join("java").join("Basic.java");
    let basic_log = Path::new("tests")
        .join("resources")
        .join("java")
        .join("basic.log");
    cmd.arg("-d")
        .arg(basic_source.to_str().expect("test case source code exists"))
        .arg("-l")
        .arg(basic_log.to_str().expect("test case log exists"));
    cmd.assert().success().stdout(r#"{"srcRef":{"sourcePath":"tests/java/Basic.java","lineNumber":18,"column":16,"name":"main","text":"\"Hello from main\"","vars":[]},"variables":{},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/Basic.java","lineNumber":25,"column":20,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"0"},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/Basic.java","lineNumber":25,"column":20,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"1"},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/Basic.java","lineNumber":25,"column":20,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"2"},"stack":[]}
"#);
    Ok(())
}

#[test]
fn basic_with_log() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("log2src")?;
    let basic_source = Path::new("tests").join("java").join("BasicWithLog.java");
    let basic_log = Path::new("tests")
        .join("resources")
        .join("java")
        .join("basic.log");
    cmd.arg("-d")
        .arg(basic_source.to_str().expect("test case source code exists"))
        .arg("-l")
        .arg(basic_log.to_str().expect("test case log exists"));
    cmd.assert().success().stdout(r#"{"srcRef":{"sourcePath":"tests/java/BasicWithLog.java","lineNumber":18,"column":13,"name":"main","text":"\"Hello from main\"","vars":[]},"variables":{},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/BasicWithLog.java","lineNumber":25,"column":17,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"0"},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/BasicWithLog.java","lineNumber":25,"column":17,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"1"},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/BasicWithLog.java","lineNumber":25,"column":17,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"2"},"stack":[]}
"#);
    Ok(())
}

#[test]
fn basic_with_upper() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("log2src")?;
    let basic_source = Path::new("tests").join("java").join("BasicWithUpper.java");
    let basic_log = Path::new("tests")
        .join("resources")
        .join("java")
        .join("basic.log");
    cmd.arg("-d")
        .arg(basic_source.to_str().expect("test case source code exists"))
        .arg("-l")
        .arg(basic_log.to_str().expect("test case log exists"));
    cmd.assert().success().stdout(r#"{"srcRef":{"sourcePath":"tests/java/BasicWithUpper.java","lineNumber":18,"column":16,"name":"main","text":"\"Hello from main\"","vars":[]},"variables":{},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/BasicWithUpper.java","lineNumber":25,"column":20,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"0"},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/BasicWithUpper.java","lineNumber":25,"column":20,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"1"},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/BasicWithUpper.java","lineNumber":25,"column":20,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"2"},"stack":[]}
"#);
    Ok(())
}

#[test]
fn basic_with_log_format() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("log2src")?;
    let source = Path::new("tests").join("java").join("BasicWithCustom.java");
    let log = Path::new("tests")
        .join("resources")
        .join("java")
        .join("basic-class-line.log");
    cmd.arg("-d")
        .arg(source.to_str().expect("test case source code exists"))
        .arg("-l")
        .arg(log.to_str().expect("test case log exists"))
        .arg("-f")
        .arg("^(?<timestamp>\\d{4}-\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}) (?<level>\\w+) (?<name>[\\w$.]+):(?<line>\\d+) (?<method>[\\w$]+): (?<body>.*)$");
    cmd.assert().success().stdout(r#"{"srcRef":{"sourcePath":"tests/java/BasicWithCustom.java","lineNumber":15,"column":16,"name":"main","text":"\"Hello from main\"","vars":[]},"variables":{},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/BasicWithCustom.java","lineNumber":22,"column":20,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"0"},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/BasicWithCustom.java","lineNumber":22,"column":20,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"1"},"stack":[]}
{"srcRef":{"sourcePath":"tests/java/BasicWithCustom.java","lineNumber":22,"column":20,"name":"foo","text":"\"Hello from foo i=\\{i}\"","vars":["i"]},"variables":{"i":"2"},"stack":[]}
"#);
    Ok(())
}
