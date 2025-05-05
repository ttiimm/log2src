use std::{path::Path, process::Command};

use serde_json::{from_str, Value};

pub fn assert_source_ref_output(
    cmd: &mut Command,
    expected_str: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Command failed with error: {}", stderr).into());
    }

    let output_str = String::from_utf8(output.stdout)?;
    let output_json = to_json(output_str.clone());
    let expected_json = to_json(expected_str.to_owned());

    if output_json.len() != expected_json.len() {
        return Err(format!(
            "Expected {} JSON objects, but got {}.\nExpected: {}\nActual: {}",
            expected_json.len(),
            output_json.len(),
            expected_str,
            output_str
        )
        .into());
    }

    for (i, (actual, expected)) in output_json.iter().zip(expected_json.iter()).enumerate() {
        let mut actual = actual.clone();
        let mut expected = expected.clone();

        normalize_src_ref(&mut actual);
        normalize_src_ref(&mut expected);

        if actual != expected {
            return Err(format!(
                "JSON object #{} doesn't match.\nExpected: {}\nActual: {}",
                i, expected, actual
            )
            .into());
        }
    }

    Ok(())
}

fn to_json(text: String) -> Vec<Value> {
    text.lines()
        .filter_map(|line| from_str(line).ok())
        .collect()
}

fn normalize_src_ref(value: &mut Value) {
    if let Some(src_ref) = value.get_mut("srcRef") {
        if let Some(obj) = src_ref.as_object_mut() {
            if let Some(path) = obj.get_mut("sourcePath") {
                norm_src_path(path);
            }
        }
    }

    if let Some(src_ref) = value.get_mut("srcRef") {
        if let Some(obj) = src_ref.as_object_mut() {
            if let Some(stack) = obj.get_mut("stack") {
                for call_stack in stack.as_array_mut() {
                    for stack_item in call_stack {
                        if let Some(obj) = stack_item.as_object_mut() {
                            if let Some(path) = obj.get_mut("sourcePath") {
                                norm_src_path(path);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn norm_src_path(src_path: &mut Value) {
    if let Some(path_str) = src_path.as_str() {
        // Convert the path to the platform's format
        let path_sep = std::path::MAIN_SEPARATOR;
        let normalized = if path_sep == '/' {
            path_str.to_string()
        } else {
            // On Windows, swap the forward slash for backslashes
            path_str.replace('/', &path_sep.to_string())
        };
        *src_path = Value::String(normalized);
    }
}
