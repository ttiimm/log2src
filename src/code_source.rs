use std::{
    ffi::OsStr,
    fs::{self, File},
    io,
    path::PathBuf,
};

use tree_sitter::Language;

use crate::SourceLanguage;

pub struct CodeSource {
    pub(crate) filename: String,
    pub(crate) language: SourceLanguage,
    pub(crate) buffer: String,
}

impl CodeSource {
    pub fn new(path: PathBuf, mut input: Box<dyn io::Read>) -> CodeSource {
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

    pub fn ts_language(&self) -> Language {
        match self.language {
            SourceLanguage::Rust => tree_sitter_rust::LANGUAGE.into(),
            SourceLanguage::Java => tree_sitter_java::LANGUAGE.into(),
        }
    }

    pub fn find_code(sources: &str, filter: Option<Vec<String>>) -> Vec<CodeSource> {
        let mut srcs = vec![];
        let meta = fs::metadata(sources).expect("can read file metadata");
        if meta.is_file() {
            let path = PathBuf::from(sources);
            try_add_file(path, &mut srcs, &filter);
        } else {
            walk_dir(PathBuf::from(sources), &mut srcs, &filter).expect("can traverse directory");
        }
        srcs
    }
}

const SUPPORTED_EXTS: &[&str] = &["java", "rs"];

fn try_add_file(path: PathBuf, srcs: &mut Vec<CodeSource>, filter: &Option<Vec<String>>) {
    if let Some(filter_list) = filter {
        if let Some(file_name) = path.file_name() {
            if !filter_list
                .iter()
                .any(|f| file_name.to_string_lossy().contains(f))
            {
                return;
            }
        }
    };

    let ext = path.extension().unwrap_or(OsStr::new(""));
    if SUPPORTED_EXTS.iter().any(|&supported| supported == ext) {
        let input = Box::new(File::open(PathBuf::from(&path)).expect("can open file"));
        let code = CodeSource::new(path, input);
        srcs.push(code);
    }
}

fn walk_dir(
    dir: PathBuf,
    srcs: &mut Vec<CodeSource>,
    filter: &Option<Vec<String>>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            try_add_file(path, srcs, filter);
        } else if metadata.is_dir() {
            walk_dir(path, srcs, filter).expect("can traverse directory");
        }
    }
    Ok(())
}
