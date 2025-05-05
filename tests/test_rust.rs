use assert_cmd::prelude::*;
use std::{path::Path, process::Command};

mod utils;
use utils::assert_source_ref_output;

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
        .arg(log.to_str().expect("test case log path is valid"));

    assert_source_ref_output(
        &mut cmd,
        r#"{"srcRef":{"sourcePath":"examples/basic.rs","lineNumber":6,"column":11,"name":"main","text":"\"Hello from main\"","vars":[]},"variables":{},"stack":[[{"sourcePath":"examples/basic.rs","lineNumber":8,"column":8,"name":"main","text":"foo","vars":[]}]]}
{"srcRef":{"sourcePath":"examples/basic.rs","lineNumber":13,"column":11,"name":"foo","text":"\"Hello from foo i={}\"","vars":["i"]},"variables":{"i":"0"},"stack":[[{"sourcePath":"examples/basic.rs","lineNumber":8,"column":8,"name":"main","text":"foo","vars":[]}]]}
{"srcRef":{"sourcePath":"examples/basic.rs","lineNumber":13,"column":11,"name":"foo","text":"\"Hello from foo i={}\"","vars":["i"]},"variables":{"i":"1"},"stack":[[{"sourcePath":"examples/basic.rs","lineNumber":8,"column":8,"name":"main","text":"foo","vars":[]}]]}
{"srcRef":{"sourcePath":"examples/basic.rs","lineNumber":13,"column":11,"name":"foo","text":"\"Hello from foo i={}\"","vars":["i"]},"variables":{"i":"2"},"stack":[[{"sourcePath":"examples/basic.rs","lineNumber":8,"column":8,"name":"main","text":"foo","vars":[]}]]}
"#,
    )?;
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
        .arg("-s")
        .arg("1");

    assert_source_ref_output(
        &mut cmd,
        r#"{"srcRef":{"sourcePath":"examples/stack.rs","lineNumber":15,"column":11,"name":"b","text":"\"Hello from b\"","vars":[]},"variables":{},"stack":[[{"sourcePath":"examples/stack.rs","lineNumber":11,"column":4,"name":"a","text":"b","vars":[]},{"sourcePath":"examples/stack.rs","lineNumber":7,"column":4,"name":"main","text":"a","vars":[]}]]}
"#,
    )?;
    Ok(())
}
