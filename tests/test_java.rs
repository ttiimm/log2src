use std::path::Path;

#[test]
fn basics() {
    let basic_log = Path::new("tests")
        .join("resources")
        .join("java")
        .join("basic.log");
    assert!(basic_log.exists())
}
