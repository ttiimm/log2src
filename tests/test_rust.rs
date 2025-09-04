use assert_cmd::prelude::*;
use std::{path::Path, process::Command};
use insta_cmd::assert_cmd_snapshot;

fn get_platform_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unix"
    }
}

#[test]
fn basic() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("log2src")?;
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

    let snapshot_name = format!("basic_{}", get_platform_suffix());
    assert_cmd_snapshot!(snapshot_name, cmd);

    Ok(())
}

#[test]
fn stack() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("log2src")?;
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

    let snapshot_name = format!("stack_{}", get_platform_suffix());
    assert_cmd_snapshot!(snapshot_name, cmd);
    Ok(())
}
