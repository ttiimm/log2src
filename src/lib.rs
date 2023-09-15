use regex::Regex;

pub fn filter_log(buffer: &String, thread_re: Regex) -> Vec<(&str, &str)> {
    let results: Vec<(&str, &str)> = buffer.lines()
        .filter_map(|line| {
            match thread_re.captures(line) {
                Some(capture) => Some((capture.get(0).unwrap().as_str(), line)),
                _ => None
            }
        })
        .collect();
    results
}