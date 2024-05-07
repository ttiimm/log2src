use std::process::Command;
use std::str;

fn main() {
    let java = Command::new("java")
        .arg("--enable-preview")
        .arg("--source 22")
        .arg("tests/java/Basic.java")
        .output()
        .expect("failed to run Java test example, install a JDK");

    if !java.status.success() {
        println!("stdout: {}", str::from_utf8(&java.stdout).unwrap());
        println!("stderr: {}", str::from_utf8(&java.stderr).unwrap());
        panic!("failed to run Java code, fix compilation errors");
    }
}