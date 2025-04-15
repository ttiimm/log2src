use std::ops::Range;

use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Range as TSRange, StreamingIterator, Tree};

use crate::CodeSource;

pub struct SourceQuery<'a> {
    pub source: &'a str,
    tree: Tree,
    language: Language,
}

pub(crate) struct QueryResult {
    pub kind: String,
    pub range: TSRange,
    pub name_range: Range<usize>,
}

impl<'a> SourceQuery<'a> {
    pub fn new(code: &'a CodeSource) -> SourceQuery<'a> {
        // println!("{}", code.filename);
        let mut parser = Parser::new();
        let language = code.ts_language();
        parser
            .set_language(&language)
            .expect(format!("Error loading {:?} grammar", language).as_str());
        let source = code.buffer.as_str();
        let tree = parser.parse(source, None).expect("source is parsable");
        // println!("{:?}", tree.root_node().to_sexp());
        SourceQuery {
            source,
            tree,
            language,
        }
    }

    pub(crate) fn query(&self, query: &str, node_kind: Option<&str>) -> Vec<QueryResult> {
        let query = Query::new(&self.language, query).unwrap();
        let filter_idx = node_kind.and_then(|kind| query.capture_index_for_name(kind));
        let mut cursor = QueryCursor::new();
        let mut results = Vec::new();
        let matches = cursor.matches(&query, self.tree.root_node(), self.source.as_bytes());
        matches.for_each(|m| {
            for capture in m.captures {
                if filter_idx.is_none() || (filter_idx.is_some() && filter_idx.unwrap() == capture.index) {
                    results.push(QueryResult {
                        kind: String::from(capture.node.kind()),
                        range: capture.node.range(),
                        name_range: self.find_fn_range(capture.node),
                    });
                }
            }
        });
        
        results
    }

    fn find_fn_range(&self, node: Node) -> Range<usize> {
        // println!("node.kind()={:?}", node.kind());
        match node.kind() {
            "function_item" => {
                let range = node.child_by_field_name("name").unwrap().range();
                range.start_byte..range.end_byte
            }
            "method_declaration" => {
                let range = node.child_by_field_name("name").unwrap().range();
                range.start_byte..range.end_byte
            }
            "constructor_declaration" => {
                let range = node.child_by_field_name("name").unwrap().range();
                range.start_byte..range.end_byte
            }
            "class_declaration" => {
                let range = node.child_by_field_name("name").unwrap().range();
                range.start_byte..range.end_byte
            }
            _ => {
                let r = self.find_fn_range(node.parent().unwrap());
                // println!("*****");
                r
            }
        }
    }
}
