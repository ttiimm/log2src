use std::ops::Range;

use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Range as TSRange, Tree};

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
        let filter_idx = node_kind.map_or(None, |kind| query.capture_index_for_name(kind));
        let mut cursor = QueryCursor::new();
        cursor
            .matches(&query, self.tree.root_node(), self.source.as_bytes())
            .into_iter()
            .flat_map(|m| m.captures)
            .filter(|c| {
                filter_idx.is_none() || (filter_idx.is_some() && filter_idx.unwrap() == c.index)
            })
            .map(|c| QueryResult {
                kind: String::from(c.node.kind()),
                range: c.node.range(),
                name_range: self.find_fn_range(c.node),
            })
            .collect()
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
