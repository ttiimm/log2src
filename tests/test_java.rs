use assert_cmd::prelude::*;
use insta_cmd::assert_cmd_snapshot;
use std::{path::Path, process::Command};

mod common_settings;

#[test]
fn basic() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = common_settings::enable_filters();
    let mut cmd = Command::cargo_bin("log2src")?;
    let basic_source = Path::new("tests").join("java").join("Basic.java");
    let basic_log = Path::new("tests")
        .join("resources")
        .join("java")
        .join("basic.log");
    cmd.arg("-d")
        .arg(basic_source.to_str().expect("test case source code exists"))
        .arg("-l")
        .arg(basic_log.to_str().expect("test case log exists"))
        .arg("-f")
        .arg(r#"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} \w+ \w+ \w+: (?<body>.*)"#);

    assert_cmd_snapshot!(cmd);
    Ok(())
}

#[test]
fn basic_with_log() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = common_settings::enable_filters();
    let mut cmd = Command::cargo_bin("log2src")?;
    let basic_source = Path::new("tests").join("java").join("BasicWithLog.java");
    let basic_log = Path::new("tests")
        .join("resources")
        .join("java")
        .join("basic.log");
    cmd.arg("-d")
        .arg(basic_source.to_str().expect("test case source code exists"))
        .arg("-l")
        .arg(basic_log.to_str().expect("test case log exists"))
        .arg("-f")
        .arg(r#"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} \w+ \w+ \w+: (?<body>.*)"#);

    assert_cmd_snapshot!(cmd);
    Ok(())
}

#[test]
fn basic_with_upper() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = common_settings::enable_filters();
    let mut cmd = Command::cargo_bin("log2src")?;
    let basic_source = Path::new("tests").join("java").join("BasicWithUpper.java");
    let basic_log = Path::new("tests")
        .join("resources")
        .join("java")
        .join("basic.log");
    cmd.arg("-d")
        .arg(basic_source.to_str().expect("test case source code exists"))
        .arg("-l")
        .arg(basic_log.to_str().expect("test case log exists"))
        .arg("-f")
        .arg(r#"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} \w+ \w+ \w+: (?<body>.*)"#);

    assert_cmd_snapshot!(cmd);
    Ok(())
}

#[test]
fn basic_with_log_format() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = common_settings::enable_filters();
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
        .arg("^(?<timestamp>\\d{4}-\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}) (?<level>\\w+) (?<file>[\\w$.]+):(?<line>\\d+) (?<method>[\\w$]+): (?<body>.*)$");

    assert_cmd_snapshot!(cmd);
    Ok(())
}

#[test]
fn basic_slf4j() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = common_settings::enable_filters();
    let mut cmd = Command::cargo_bin("log2src")?;
    let source = Path::new("tests").join("java").join("BasicSlf4j.java");
    let log = Path::new("tests")
        .join("resources")
        .join("java")
        .join("basic-slf4j.log");
    cmd.arg("-d")
        .arg(source.to_str().expect("test case source code exists"))
        .arg("-l")
        .arg(log.to_str().expect("test case log exists"))
        .arg("-f")
        .arg("^(?<timestamp>\\d{4}-\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}) (?<body>.*)$");

    assert_cmd_snapshot!(cmd);
    Ok(())
}
