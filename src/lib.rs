use regex::Regex;
use serde::Serialize;
#[cfg(test)]
use std::ptr;
use std::{
    collections::HashMap,
    ffi::OsStr,
    fmt,
    fs::{self, File},
    io,
    ops::Range,
    path::PathBuf,
};
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

#[derive(Debug, PartialEq)]
enum SourceLanguage {
    Rust,
    Java,
}

const IDENTS_RS: &[&str] = &["debug", "info", "warn"];
const IDENTS_JAVA: &[&str] = &["logger", "log", "fine", "debug", "info", "warn"];

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
                    (method_invocation 
                        object: (identifier) @object-name
                        name: (identifier) @method-name
                        arguments: (argument_list [
                            (_ (string_literal) @log  (_ (identifier) @arguments))
                            (_ (string_literal (_ (identifier)* @arguments)) @log)
                            (string_literal) @log (identifier) @arguments
                            (string_literal) @log
                        ])
                        (#match? @object-name "log(ger)?|LOG(GER)?")
                        (#match? @method-name "fine|debug|info|warn")
                    )
                "#
            }
        }
    }

    fn get_identifiers(&self) -> &[&str] {
        match self {
            SourceLanguage::Rust => IDENTS_RS,
            SourceLanguage::Java => IDENTS_JAVA,
        }
    }
}

pub struct CodeSource {
    filename: String,
    language: SourceLanguage,
    buffer: String,
}

const SUPPORTED_EXTS: &[&str] = &["java", "rs"];

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
    let meta = fs::metadata(sources).expect("can read file metadata");
    if meta.is_file() {
        let path = PathBuf::from(sources);
        try_add_file(path, &mut srcs);
    } else {
        walk_dir(PathBuf::from(sources), &mut srcs).expect("can traverse directory");
    }
    srcs
}

fn walk_dir(dir: PathBuf, srcs: &mut Vec<CodeSource>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            try_add_file(path, srcs);
        } else if metadata.is_dir() {
            walk_dir(path, srcs).expect("can traverse directory");
        }
    }
    Ok(())
}

fn try_add_file(path: PathBuf, srcs: &mut Vec<CodeSource>) {
    let ext = path.extension().unwrap_or(OsStr::new(""));
    if SUPPORTED_EXTS.iter().any(|&supported| supported == ext) {
        let input = Box::new(File::open(PathBuf::from(&path)).expect("can open file"));
        let code = CodeSource::new(path, input);
        srcs.push(code);
    }
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
    pub line: &'a str,
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
            },
            "class_declaration" => {
                let range = node.child_by_field_name("name").unwrap().range();
                range.start_byte..range.end_byte
            }
            _ => {
                let r = self.find_fn_range(node.parent().unwrap());
                // println!("*****");
                r
            },
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
    edges: Vec<Edge<'a>>,
}

#[derive(Debug, PartialEq)]
pub struct Edge<'a> {
    // same as SourceRef found in via
    // from: &'a str,
    to: &'a str,
    via: SourceRef,
}

impl<'a> CallGraph<'a> {
    pub fn new(sources: &'a mut Vec<CodeSource>) -> CallGraph<'a> {
        let edges = Self::find_edges(sources);
        CallGraph { edges }
    }

    fn find_edges(sources: &'a mut Vec<CodeSource>) -> Vec<Edge<'a>> {
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
                    let src_ref = build_src_ref(code, result);

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

pub fn link_to_source<'a>(log_ref: &LogRef, src_refs: &'a Vec<SourceRef>) -> Option<&'a SourceRef> {
    src_refs.iter().find(|&source_ref| {
        if let Some(_) = source_ref.matcher.captures(log_ref.line) {
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
        if let Some(captures) = src_ref.matcher.captures(log_line.line) {
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
                Some(LogRef { line })
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
    call_graph: &'a CallGraph,
) -> Vec<LogMapping<'a>> {
    log_refs
        .iter()
        .map(|log_ref| {
            let src_ref: Option<&SourceRef> = link_to_source(&log_ref, &src_logs);
            let variables = src_ref.map_or(HashMap::new(), |src_ref| {
                extract_variables(&log_ref, src_ref)
            });
            let stack = src_ref.map_or(Vec::new(), |src_ref| {
                find_possible_paths(src_ref, &call_graph)
            });
            LogMapping {
                log_ref,
                src_ref,
                variables,
                stack,
            }
        })
        .collect::<Vec<LogMapping>>()
}

pub fn find_possible_paths<'a>(
    src_ref: &'a SourceRef,
    call_graph: &'a CallGraph,
) -> Vec<Vec<&'a SourceRef>> {
    let mut possible = Vec::new();
    let mains = call_graph
        .edges
        .iter()
        .filter(|edge| edge.via.name == "main")
        .collect::<Vec<&Edge>>();
    for main in mains.into_iter() {
        let mut stack = vec![main];
        let mut visited = vec![main];
        while let Some(next) = stack.pop() {
            if next.to == src_ref.name {
                break;
            }

            let candidates = call_graph
                .edges
                .iter()
                .filter(|edge| next.to == edge.via.name)
                .collect::<Vec<&Edge>>();

            for edge in candidates {
                if !visited.contains(&edge) {
                    stack.push(edge);
                    visited.push(edge);
                }
            }
        }
        possible.push(
            visited
                .iter()
                .rev()
                .map(|edge| &edge.via)
                .collect::<Vec<&SourceRef>>(),
        );
    }

    possible
}

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
                    // check the text doesn't match any of the identifiers we're looking for
                    if code
                        .language
                        .get_identifiers()
                        .iter()
                        .all(|&s| s != text.to_lowercase())
                    {
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
    let unquoted = &source[start..end].to_string();
    // println!("{} line {}", code.filename, line);
    let matcher = build_matcher(unquoted);
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

fn build_matcher(text: &str) -> Regex {
    if text == "{}" || text.trim() == "" {
        Regex::new("foo").unwrap()
    } else {
        let curly_replacer = Regex::new(r#"\\?\{.*?\}"#).unwrap();
        let escaped = curly_replacer
            .split(text)
            .map(|s| regex::escape(s))
            .collect::<Vec<String>>()
            .join(r#"(\w+?)"#);
        Regex::new(&escaped).unwrap()
    }
}

#[test]
fn test_filter_log_defaults() {
    let buffer = String::from("hello\nwarning\nerror\nboom");
    let result = filter_log(&buffer, Filter::default());
    assert_eq!(
        result,
        vec![
            LogRef { line: "hello" },
            LogRef { line: "warning" },
            LogRef { line: "error" },
            LogRef { line: "boom" }
        ]
    );
}

#[test]
fn test_filter_log_with_filter() {
    let buffer = String::from("hello\nwarning\nerror\nboom");
    let result = filter_log(&buffer, Filter { start: 1, end: 2 });
    assert_eq!(result, vec![LogRef { line: "warning" }]);
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
        line: "[2024-02-15T03:46:44Z DEBUG stack] you're only as funky as your last cut",
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
        line: "[2024-02-26T03:44:40Z DEBUG stack] nope!",
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
        line: "[2024-02-15T03:46:44Z DEBUG nope] this won't match i=1",
    };
    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let src_refs = extract_logging(&mut vec![code]);
    assert_eq!(src_refs.len(), 2);
    let vars = extract_variables(&log_ref, &src_refs[1]);
    assert_eq!(vars.get("i"), Some(&"1"));
}

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

#[test]
fn test_find_possible_paths() {
    let code = CodeSource::new(PathBuf::from("in-mem.rs"), Box::new(TEST_SOURCE.as_bytes()));
    let mut sources = vec![code];
    let src_refs = extract_logging(&mut sources);
    let call_graph = CallGraph::new(&mut sources);
    let paths = find_possible_paths(&src_refs[1], &call_graph);

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
    assert_eq!(paths, vec![vec![&foo_2_nope, &main_2_foo]])
}

#[test]
fn test_build_matcher_curlies() {
    let matcher = build_matcher("{}) {}, {}");
    assert_eq!(
        Regex::new(r#"(\w+)\) (\w+), (\w+)"#).unwrap().as_str(),
        matcher.as_str()
    );
}

#[test]
fn test_build_matcher_mix() {
    let matcher = build_matcher("{}) {:?}, {foo.bar}");
    assert_eq!(
        Regex::new(r#"(\w+)\) (\w+), (\w+)"#).unwrap().as_str(),
        matcher.as_str()
    );
}
