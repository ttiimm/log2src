use regex::Regex;
use serde::Serialize;
#[cfg(test)]
use std::ptr;
use std::{collections::HashMap, fmt, fs, io, ops::Range, path::PathBuf};
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, Range as TSRange, Tree};

pub struct Filter {
    pub start: usize,
    pub end: usize,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            start: 0,
            end: usize::MAX,
        }
    }
}

enum SourceLanguage {
    Rust,
    Java,
}
impl SourceLanguage {
    fn get_query(&self) -> &str {
        match self {
            SourceLanguage::Rust => {
                // XXX: assumes it's a debug macro
                r#"
                (macro_invocation macro: (identifier) @macro-name
                    (token_tree
                        (string_literal) @log (identifier)* @arguments
                    ) (#eq? @macro-name "debug")
                )
            "#
            }
            SourceLanguage::Java => {
                r#"
                    (method_invocation object: (identifier) @object-name
                        name: (identifier) @method-name
                        arguments: (argument_list [
                            (string_literal) @log
                            (_ (string_literal (_ (identifier)* @arguments)) @log)
                        ])
                        (#eq? @object-name "logger")
                        (#eq? @method-name "fine")
                    )
                "#
            }
        }
    }
}

pub struct CodeSource {
    filename: String,
    language: SourceLanguage,
    buffer: String,
}

impl CodeSource {
    fn new(path: PathBuf, mut input: Box<dyn io::Read>) -> CodeSource {
        let language = match path.extension() {
            Some(ext) => match ext.to_str().unwrap() {
                "rs" => SourceLanguage::Rust,
                "java" => SourceLanguage::Java,
                _ => panic!("Unsupported language"),
            },
            None => panic!("No extension"),
        };
        let mut buffer = String::new();
        input.read_to_string(&mut buffer).expect("can read source");
        CodeSource {
            language,
            filename: path.to_string_lossy().to_string(),
            buffer,
        }
    }

    fn ts_language(&self) -> Language {
        match self.language {
            SourceLanguage::Rust => tree_sitter_rust::language(),
            SourceLanguage::Java => tree_sitter_java::language(),
        }
    }
}

pub fn find_code(sources: &str) -> Vec<CodeSource> {
    let mut srcs = vec![];
    let meta = fs::metadata(sources).expect("Can read file metadata");
    if meta.is_file() {
        let path = PathBuf::from(sources);
        let input = Box::new(fs::File::open(PathBuf::from(sources)).expect("Can open file"));
        let code = CodeSource::new(path, input);
        srcs.push(code);
    } else {
        walk_dir(PathBuf::from(sources), &mut srcs).expect("Couldn't traverse directory");
    }
    srcs
}

fn walk_dir(dir: PathBuf, srcs: &mut Vec<CodeSource>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            let path = entry.path();
            let input =
                Box::new(fs::File::open(PathBuf::from(entry.path())).expect("Can open file"));
            let code = CodeSource::new(path, input);
            srcs.push(code)
        } else if metadata.is_dir() {
            walk_dir(path, srcs).expect("Couldn't traverse directory");
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct LogMapping<'a> {
    #[serde(skip_serializing)]
    pub log_ref: &'a LogRef<'a>,
    #[serde(rename(serialize = "srcRef"))]
    pub src_ref: Option<&'a SourceRef>,
    pub variables: HashMap<&'a str, &'a str>,
    pub stack: Vec<Vec<&'a SourceRef>>,
}

#[derive(Debug, PartialEq)]
pub struct LogRef<'a> {
    pub text: &'a str,
}

pub struct QueryResult {
    kind: String,
    range: TSRange,
    name_range: Range<usize>,
}

pub struct SourceQuery<'a> {
    pub source: &'a str,
    tree: Tree,
    language: Language,
}

impl<'a> SourceQuery<'a> {
    pub fn new(code: &'a CodeSource) -> SourceQuery<'a> {
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

    pub fn query(&self, query: &str, node_kind: Option<&str>) -> Vec<QueryResult> {
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
                name_range: self.find_fn_range(c.node, self.source),
            })
            .collect()
    }

    fn find_fn_range(&self, node: Node, source: &'a str) -> Range<usize> {
        match node.kind() {
            "function_item" => {
                let range = node.child_by_field_name("name").unwrap().range();
                range.start_byte..range.end_byte
            }
            "method_declaration" => {
                let range = node.child_by_field_name("name").unwrap().range();
                range.start_byte..range.end_byte
            }
            _ => self.find_fn_range(node.parent().unwrap(), source),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SourceRef {
    #[serde(rename(serialize = "sourcePath"))]
    source_path: String,
    #[serde(rename(serialize = "lineNumber"))]
    pub line_no: usize,
    column: usize,
    name: String,
    text: String,
    #[serde(skip_serializing)]
    matcher: Regex,
    vars: Vec<String>,
}

impl fmt::Display for SourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[Line: {}, Col: {}] source `{}` name `{}` vars={:?}",
            self.line_no, self.column, self.text, self.name, self.vars
        )
    }
}

impl PartialEq for SourceRef {
    fn eq(&self, other: &Self) -> bool {
        self.line_no == other.line_no
            && self.column == other.column
            && self.name == other.name
            && self.text == other.text
            && self.vars == other.vars
    }
}

#[derive(Debug)]
pub struct CallGraph<'a> {
    _nodes: Vec<&'a str>,
    edges: Vec<Edge<'a>>,
}

#[derive(Debug, PartialEq)]
pub struct Edge<'a> {
    // same as SourceRef found in via
    // from: &'a str,
    to: &'a str,
    via: SourceRef,
}

// impl<'a> CallGraph<'a> {
//     pub fn new(src_query: &'a SourceQuery) -> CallGraph<'a> {
//         let _nodes = Self::find_nodes(src_query);
//         let edges = Self::find_edges(src_query);
//         CallGraph { _nodes, edges }
//     }

//     fn find_nodes<'b>(src_query: &'a SourceQuery) -> Vec<&'a str> {
//         let node_query = r#"
//             (function_item name: (identifier) @fn_name parameters: (parameters)*)
//         "#;
//         let results = src_query.query(node_query, Some("fn_name"));
//         let mut symbols = Vec::new();
//         for result in results {
//             symbols.push(&src_query.source[result.name_range]);
//         }
//         symbols
//     }

//     fn find_edges(src_query: &'a SourceQuery) -> Vec<Edge<'a>> {
//         let edge_query = r#"
//             (call_expression function: (identifier) @fn_name arguments: (arguments (_))*)
//         "#;
//         let results = src_query.query(edge_query, Some("fn_name"));
//         let mut symbols = Vec::new();
//         for result in results {
//             let range = result.range;
//             let fn_call = &src_query.source[range.start_byte..range.end_byte];
//             let src_ref = build_src_ref(&src_query.source, result);

//             symbols.push(Edge {
//                 to: fn_call,
//                 via: src_ref,
//             });
//         }
//         symbols
//     }
// }

pub fn link_to_source<'a>(log_ref: &LogRef, src_refs: &'a Vec<SourceRef>) -> Option<&'a SourceRef> {
    src_refs.iter().find(|&source_ref| {
        if let Some(_) = source_ref.matcher.captures(log_ref.text) {
            return true;
        }
        false
    })
}

pub fn extract_variables<'a>(
    log_line: &'a LogRef,
    src_ref: &'a SourceRef,
) -> HashMap<&'a str, &'a str> {
    let mut variables = HashMap::new();
    if src_ref.vars.len() > 0 {
        if let Some(captures) = src_ref.matcher.captures(log_line.text) {
            for i in 0..captures.len() - 1 {
                variables.insert(
                    src_ref.vars[i].as_str(),
                    captures.get(i + 1).unwrap().as_str(),
                );
            }
        }
    }

    variables
}

pub fn filter_log(buffer: &String, filter: Filter) -> Vec<LogRef> {
    let results = buffer
        .lines()
        .enumerate()
        .filter_map(|(line_no, line)| {
            if filter.start <= line_no && line_no < filter.end {
                Some(LogRef { text: line })
            } else {
                None
            }
        })
        .collect();
    results
}

pub fn do_mappings<'a>(
    log_refs: &'a Vec<LogRef>,
    src_logs: &'a Vec<SourceRef>,
) -> Vec<LogMapping<'a>> {
    log_refs
        .iter()
        .map(|log_ref| {
            let src_ref: Option<&SourceRef> = link_to_source(&log_ref, &src_logs);
            let variables = src_ref.map_or(HashMap::new(), |src_ref| {
                extract_variables(&log_ref, src_ref)
            });
            let stack = Vec::new();
            // let stack = src_ref.map_or(Vec::new(), |src_ref| {
            //     find_possible_paths(src_ref, &call_graph)
            // });
            LogMapping {
                log_ref,
                src_ref,
                variables,
                stack,
            }
        })
        .collect::<Vec<LogMapping>>()
}

// pub fn find_possible_paths<'a>(
//     src_ref: &'a SourceRef,
//     call_graph: &'a CallGraph,
// ) -> Vec<Vec<&'a SourceRef>> {
//     let mut possible = Vec::new();
//     let mut visited = Vec::new();
//     for main_edge in call_graph.edges.iter().filter(|e| e.via.name == "main") {
//         let mut path = Vec::new();
//         path.push(&main_edge.via);
//         viable_path(
//             &main_edge.to,
//             &src_ref.name,
//             call_graph,
//             &mut possible,
//             &mut visited,
//             &mut path,
//         );
//     }
//     possible
// }

// fn viable_path<'a>(
//     node: &'a str,
//     target: &'a str,
//     call_graph: &'a CallGraph,
//     possible: &mut Vec<Vec<&SourceRef>>,
//     visited: &mut Vec<&'a str>,
//     path: &mut Vec<&'a SourceRef>,
// ) {
//     if visited.contains(&node) {
//         return;
//     }
//     visited.push(node);

//     if node == target {
//         let mut found_path = path.to_vec();
//         found_path.reverse();
//         possible.push(found_path);
//         return;
//     }

//     for next_edge in call_graph.edges.iter().filter(|e| e.via.name == node) {
//         path.push(&next_edge.via);
//         viable_path(&next_edge.to, target, call_graph, possible, visited, path);
//         path.pop();
//     }
// }

pub fn extract_logging<'a>(sources: &mut Vec<CodeSource>) -> Vec<SourceRef> {
    let mut matched = Vec::new();
    for code in sources.iter() {
        let src_query = SourceQuery::new(code);
        let query = code.language.get_query();
        let results = src_query.query(query, None);

        for result in results {
            match result.kind.as_str() {
                "string_literal" => {
                    let src_ref = build_src_ref(code, result);
                    matched.push(src_ref);
                }
                "identifier" => {
                    let range = result.range;
                    let source = code.buffer.as_str();
                    let text = source[range.start_byte..range.end_byte].to_string();
                    if text != "debug" && text != "logger" && text != "fine" {
                        let length = matched.len() - 1;
                        let prior_result: &mut SourceRef = matched.get_mut(length).unwrap();
                        prior_result.vars.push(text);
                    }
                }
                _ => {
                    println!("ignoring {}", result.kind)
                }
            }
        }
    }
    matched
}

fn build_src_ref<'a, 'q>(code: &CodeSource, result: QueryResult) -> SourceRef {
    let range = result.range;
    let source = code.buffer.as_str();
    let text = source[range.start_byte..range.end_byte].to_string();
    let line = range.start_point.row + 1;
    let col = range.start_point.column;
    let start = range.start_byte + 1;
    let mut end = range.end_byte - 1;
    if start == range.end_byte {
        end = range.end_byte;
    }
    let unquoted = &source[start..end];
    let mut replaced = unquoted.replace("{}", "(\\w+)");
    replaced = replaced.replace("{:?}", "(\\w+)");
    let re = Regex::new(r"\\\{.*?\}").unwrap();
    replaced = re.replace_all(&replaced, "(\\w+)").to_string();
    let matcher = Regex::new(&replaced).unwrap();
    let vars = Vec::new();
    let name = source[result.name_range].to_string();
    SourceRef {
        source_path: code.filename.clone(),
        line_no: line,
        column: col,
        name,
        text,
        matcher,
        vars,
    }
}

#[test]
fn test_filter_log_defaults() {
    let buffer = String::from("hello\nwarning\nerror\nboom");
    let result = filter_log(&buffer, Filter::default());
    assert_eq!(
        result,
        vec![
            LogRef { text: "hello" },
            LogRef { text: "warning" },
            LogRef { text: "error" },
            LogRef { text: "boom" }
        ]
    );
}

#[test]
fn test_filter_log_with_filter() {
    let buffer = String::from("hello\nwarning\nerror\nboom");
    let result = filter_log(&buffer, Filter { start: 1, end: 2 });
    assert_eq!(result, vec![LogRef { text: "warning" }]);
}

#[cfg(test)]
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
fn test_extract_logging() {
    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let src_refs = extract_logging(&mut vec![code]);
    assert_eq!(src_refs.len(), 2);
    let first = &src_refs[0];
    assert_eq!(first.line_no, 7);
    assert_eq!(first.column, 11);
    assert_eq!(first.name, "main");
    assert_eq!(first.text, "\"you're only as funky as your last cut\"");
    assert!(first.vars.is_empty());

    let second = &src_refs[1];
    assert_eq!(second.line_no, 18);
    assert_eq!(second.column, 11);
    assert_eq!(second.name, "nope");
    assert_eq!(second.text, "\"this won't match i={}\"");
    assert_eq!(second.vars[0], "i");
}

#[test]
fn test_link_to_source() {
    let log_ref = LogRef {
        text: "[2024-02-15T03:46:44Z DEBUG stack] you're only as funky as your last cut",
    };
    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let src_refs = extract_logging(&mut vec![code]);
    assert_eq!(src_refs.len(), 2);
    let result = link_to_source(&log_ref, &src_refs);
    assert!(ptr::eq(result.unwrap(), &src_refs[0]));
}

#[test]
fn test_link_to_source_no_matches() {
    let log_ref = LogRef {
        text: "[2024-02-26T03:44:40Z DEBUG stack] nope!",
    };

    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let src_refs = extract_logging(&mut vec![code]);
    assert_eq!(src_refs.len(), 2);
    let result = link_to_source(&log_ref, &src_refs);
    assert_eq!(result.is_none(), true);
}

#[test]
fn test_extract_variables() {
    let log_ref = LogRef {
        text: "[2024-02-15T03:46:44Z DEBUG nope] this won't match i=1",
    };
    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let src_refs = extract_logging(&mut vec![code]);
    assert_eq!(src_refs.len(), 2);
    let vars = extract_variables(&log_ref, &src_refs[1]);
    assert_eq!(vars.get("i"), Some(&"1"));
}

// #[test]
// fn test_call_graph() {
//     let src_query = SourceQuery::new(TEST_SOURCE);
//     let call_graph = CallGraph::new(&src_query);
//     assert_eq!(call_graph._nodes, vec!["main", "foo", "nope"]);
//     let star_regex = Regex::new(".*").unwrap();
//     let main_2_foo = SourceRef {
//         line_no: 9,
//         column: 8,
//         name: "main",
//         text: "foo",
//         matcher: star_regex,
//         vars: vec![],
//     };
//     let star_regex = Regex::new(".*").unwrap();
//     let foo_2_nope = SourceRef {
//         line_no: 14,
//         column: 4,
//         name: "foo",
//         text: "nope",
//         matcher: star_regex,
//         vars: vec![],
//     };
//     assert_eq!(
//         call_graph.edges,
//         vec![
//             Edge {
//                 to: "foo",
//                 via: main_2_foo
//             },
//             Edge {
//                 to: "nope",
//                 via: foo_2_nope
//             }
//         ]
//     )
// }

// #[test]
// fn test_find_possible_paths() {
//     let src_query = SourceQuery::new(TEST_SOURCE);
//     let call_graph = CallGraph::new(&src_query);
//     let src_refs = extract_logging(&src_query);
//     let paths = find_possible_paths(&src_refs[1], &call_graph);

//     let star_regex = Regex::new(".*").unwrap();
//     let main_2_foo = SourceRef {
//         line_no: 9,
//         column: 8,
//         name: "main",
//         text: "foo",
//         matcher: star_regex,
//         vars: vec![],
//     };
//     let star_regex = Regex::new(".*").unwrap();
//     let foo_2_nope = SourceRef {
//         line_no: 14,
//         column: 4,
//         name: "foo",
//         text: "nope",
//         matcher: star_regex,
//         vars: vec![],
//     };
//     assert_eq!(paths, vec![vec![&foo_2_nope, &main_2_foo]])
// }
