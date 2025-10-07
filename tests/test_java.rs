use std::fs;
use insta_cmd::assert_cmd_snapshot;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use walkdir::WalkDir;

mod common_settings;

#[test]
fn invalid_log_path() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
    let basic_source = Path::new("tests").join("java").join("Basic.java");
    let basic_log = Path::new("badname.log");
    cmd.arg("-d")
        .arg(basic_source.to_str().expect("test case source code exists"))
        .arg("-l")
        .arg(basic_log.to_str().expect("test case log exists"))
        .arg("-f")
        .arg(r#"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} \w+ \w+ \w+: (?<body>.*)"#);

    assert_cmd_snapshot!(cmd.cmd);
    Ok(())
}

#[test]
fn invalid_log_format() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
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
        .arg(r#"^-\d{2}-\d{2} \d{2}:\d{2}:\d{2} \w+ \w+ \w+: (?<body>.*)"#);

    assert_cmd_snapshot!(cmd.cmd);
    Ok(())
}

#[test]
fn basic() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
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

    assert_cmd_snapshot!(cmd.cmd);
    Ok(())
}

#[test]
fn basic_range() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
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
        .arg(r#"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} \w+ \w+ \w+: (?<body>.*)"#)
        .arg("-s")
        .arg("1")
        .arg("-c")
        .arg("2");

    assert_cmd_snapshot!(cmd.cmd);
    Ok(())
}

#[test]
fn basic_invalid_utf() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
    let basic_source = Path::new("tests").join("java").join("Basic.java");
    let basic_log = Path::new("tests")
        .join("resources")
        .join("java")
        .join("basic-invalid-utf.log");
    cmd.arg("-d")
        .arg(basic_source.to_str().expect("test case source code exists"))
        .arg("-l")
        .arg(basic_log.to_str().expect("test case log exists"))
        .arg("-f")
        .arg(r#"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} \w+ \w+ \w+: (?<body>.*)"#);

    assert_cmd_snapshot!(cmd.cmd);
    Ok(())
}

#[test]
fn basic_with_log() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
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

    assert_cmd_snapshot!(cmd.cmd);
    Ok(())
}

#[test]
fn basic_with_upper() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
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

    assert_cmd_snapshot!(cmd.cmd);
    Ok(())
}

#[test]
fn basic_with_log_format() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
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

    for _index in 0..2 {
        assert_cmd_snapshot!(cmd.cmd);
    }
    Ok(())
}

#[test]
#[cfg(not(windows))]
fn basic_slf4j() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
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
        .arg(
            "^(?<timestamp>\\d{4}-\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}) (?<thread>\\d+) (?<body>.*)$",
        );

    assert_cmd_snapshot!(cmd.cmd);

    // corrupt the cache entry header
    for entry in WalkDir::new(cmd.home_path()) {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with("cache.") {
            let mut buffer = Vec::new();
            {
                let mut file = File::open(entry.path())?;
                file.read_to_end(&mut buffer)?;
            }
            buffer[0] = b'0';
            let mut file = File::create(entry.path())?;
            file.write_all(&buffer)?;
        }
    }

    assert_cmd_snapshot!(cmd.cmd);

    // corrupt the cache entry content
    // XXX for some reason this doesn't work on windows
    for entry in WalkDir::new(cmd.home_path()) {
        let entry = entry?;
        if entry.file_name().to_string_lossy().starts_with("cache.") {
            let mut buffer = Vec::new();
            {
                let mut file = File::open(entry.path())?;
                file.read_to_end(&mut buffer)?;
            }
            buffer.resize(buffer.len() - 50, 0);
            fs::write(entry.path(), &buffer)?;
        }
    }

    assert_cmd_snapshot!(cmd.cmd);

    Ok(())
}
