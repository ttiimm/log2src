use std::ops::Range;

use tree_sitter::{
    Language, Node, Parser, Point, Query, QueryCursor, Range as TSRange, StreamingIterator, Tree,
};

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
        let language = code.info.language.into();
        parser
            .set_language(&language)
            .unwrap_or_else(|_| panic!("Error loading {:?} grammar", language));
        let source = code.buffer.as_str();
        let tree = parser.parse(source, None).expect("source is parsable");
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
                let mut child = capture.node;
                let mut arg_start: Option<(usize, Point)> = None;

                if filter_idx.is_none() || filter_idx.is_some_and(|f| f == capture.index) {
                    results.push(QueryResult {
                        kind: capture.node.kind().to_string(),
                        range: capture.node.range(),
                        name_range: Self::find_fn_range(child),
                    });
                }
                while let Some(next_child) = child.next_sibling() {
                    if matches!(next_child.kind(), "," | ")") {
                        if let Some(start) = arg_start {
                            results.push(QueryResult {
                                kind: "identifier".to_string(),
                                range: TSRange {
                                    start_byte: start.0,
                                    start_point: start.1,
                                    end_byte: next_child.start_byte(),
                                    end_point: next_child.start_position(),
                                },
                                name_range: Self::find_fn_range(child),
                            });
                        }
                        arg_start = Some((next_child.end_byte(), next_child.end_position()));
                    }
                    child = next_child;
                }
            }
        });

        results
    }

    fn find_fn_range(node: Node) -> Range<usize> {
        // println!("node.kind()={:?}", node.kind());
        match node.kind() {
            "function_item" => {
                let range = node.child_by_field_name("name").unwrap().range();
                range.start_byte..range.end_byte
            }
            "function_definition" => {
                let range = node.child_by_field_name("declarator").unwrap().range();
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
            "declaration_list" | "static_item" | "attribute_item" => {
                let range = node.range();
                range.start_byte..range.end_byte
            }
            _ => {
                if let Some(parent) = node.parent() {
                    if parent.kind() == "translation_unit" {
                        let range = parent.range();
                        return range.start_byte..range.end_byte;
                    }
                    Self::find_fn_range(parent)
                } else {
                    let range = node.range();

                    range.start_byte..range.end_byte
                }
            }
        }
    }
}
