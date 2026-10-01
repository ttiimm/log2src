use insta_cmd::assert_cmd_snapshot;
use std::path::Path;

mod common_settings;

#[test]
fn basic() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
    let source = Path::new("examples").join("basic.rs");
    let log = Path::new("tests")
        .join("resources")
        .join("rust")
        .join("basic.log");
    cmd.arg("-d")
        .arg(source.to_str().expect("test case path is valid"))
        .arg("-l")
        .arg(log.to_str().expect("test case log path is valid"))
        .arg("-f")
        .arg(r#"\[\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z \w+ \w+\]\s+(?<body>.*)"#);

    assert_cmd_snapshot!(cmd.cmd);
    Ok(())
}

#[test]
fn source_to_logs_summary() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
    let source = Path::new("examples").join("basic.rs");
    let log = Path::new("tests")
        .join("resources")
        .join("rust")
        .join("basic.log");
    cmd.arg("-d")
        .arg(source.to_str().expect("test case path is valid"))
        .arg("-l")
        .arg(log.to_str().expect("test case log path is valid"))
        .arg("-f")
        .arg(r#"\[\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z \w+ \w+\]\s+(?<body>.*)"#)
        .arg("--summary");

    let output = cmd.cmd.output()?;
    assert!(
        output.status.success(),
        "summary command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let usages: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    let foo = usages
        .iter()
        .find(|usage| usage["name"] == "foo")
        .expect("foo usage should be present");

    // TODO move to a snapshot if --summary sticks
    assert_eq!(foo["count"], 3);
    assert_eq!(foo["samples"].as_array().unwrap().len(), 3);
    assert_eq!(foo["samples"][0]["lineNumber"], 2);

    Ok(())
}

#[test]
fn stack() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
    let source = Path::new("examples").join("stack.rs");
    let log = Path::new("tests")
        .join("resources")
        .join("rust")
        .join("stack.log");
    cmd.arg("-d")
        .arg(source.to_str().expect("test case path is valid"))
        .arg("-l")
        .arg(log.to_str().expect("test case log path is valid"))
        .arg("-f")
        .arg(r#"\[\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z \w+ \w+\]\s+(?<body>.*)"#)
        .arg("-s")
        .arg("1");

    assert_cmd_snapshot!(cmd.cmd);
    Ok(())
}

#[test]
fn invalid_source_path() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = common_settings::CommandGuard::new()?;
    let source = Path::new("examples").join("stack.r");
    let log = Path::new("tests")
        .join("resources")
        .join("rust")
        .join("stack.log");
    cmd.arg("-d")
        .arg(source.to_str().expect("test case path is valid"))
        .arg("-l")
        .arg(log.to_str().expect("test case log path is valid"))
        .arg("-f")
        .arg(r#"\[\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z \w+ \w+\]\s+(?<body>.*)"#)
        .arg("-s")
        .arg("1");

    assert_cmd_snapshot!(cmd.cmd);
    Ok(())
}
