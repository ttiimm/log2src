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
