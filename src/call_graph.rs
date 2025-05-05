use crate::{CodeSource, SourceLanguage, SourceQuery, SourceRef};

#[derive(Debug)]
pub struct CallGraph<'a> {
    pub(crate) edges: Vec<Edge<'a>>,
}

#[derive(Debug, PartialEq)]
pub struct Edge<'a> {
    // same as SourceRef found in via
    // from: &'a str,
    pub to: &'a str,
    pub via: SourceRef,
}

impl<'a> CallGraph<'a> {
    pub fn new(sources: &'a mut Vec<CodeSource>) -> CallGraph<'a> {
        let edges = Self::find_edges(sources);
        CallGraph { edges }
    }

    pub(crate) fn find_edges(sources: &'a mut Vec<CodeSource>) -> Vec<Edge<'a>> {
        let mut symbols = Vec::new();
        let edge_query = r#"
            (call_expression function: (identifier) @fn_name arguments: (arguments (_))*)
        "#;
        for code in sources.iter() {
            if code.language == SourceLanguage::Rust {
                let src_query = SourceQuery::new(code);
                let results = src_query.query(edge_query, Some("fn_name"));

                for result in results {
                    let range = result.range;
                    let fn_call = &src_query.source[range.start_byte..range.end_byte];
                    let src_ref = SourceRef::new(code, result);

                    symbols.push(Edge {
                        to: fn_call,
                        via: src_ref,
                    });
                }
            }
        }
        symbols
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use std::path::PathBuf;

    const TEST_SOURCE: &str = r#"
#[macro_use]
extern crate log;

fn main() {
    env_logger::init();
    debug!("you're only as funky as your last cut");
    for i in 0..3 {
        foo(i);
    }
}

fn foo(i: u32) {
    nope(i);
}

fn nope(i: u32) {
    debug!("this won't match i={}", i);
}
    "#;

    #[test]
    fn test_call_graph() {
        let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
        let mut sources = vec![code];
        let call_graph = CallGraph::new(&mut sources);
        let star_regex = Regex::new(".*").unwrap();
        let main_2_foo = SourceRef {
            source_path: String::from("in-mem.rs"),
            line_no: 9,
            column: 8,
            name: String::from("main"),
            text: String::from("foo"),
            matcher: star_regex,
            vars: vec![],
        };
        let star_regex = Regex::new(".*").unwrap();
        let foo_2_nope = SourceRef {
            source_path: String::from("in-mem.rs"),
            line_no: 14,
            column: 4,
            name: String::from("foo"),
            text: String::from("nope"),
            matcher: star_regex,
            vars: vec![],
        };
        assert_eq!(
            call_graph.edges,
            vec![
                Edge {
                    to: "foo",
                    via: main_2_foo
                },
                Edge {
                    to: "nope",
                    via: foo_2_nope
                }
            ]
        )
    }
}
