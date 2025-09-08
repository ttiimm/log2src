use std::path::{Path, PathBuf};
use std::{
    fs::{self, File},
    io,
};
use tree_sitter::Language;

use crate::{LogError, SourceLanguage};

pub struct CodeSource {
    pub(crate) filename: String,
    pub(crate) language: SourceLanguage,
    pub(crate) buffer: String,
}

impl CodeSource {
    pub fn new(path: &Path, mut input: Box<dyn io::Read>) -> Result<CodeSource, LogError> {
        let language = match path.extension() {
            Some(ext) => match ext.to_str().unwrap() {
                "rs" => SourceLanguage::Rust,
                "java" => SourceLanguage::Java,
                "h" | "hh" | "hpp" | "cc" | "cpp" => SourceLanguage::Cpp,
                _ => panic!("Unsupported language"),
            },
            None => panic!("No extension"),
        };
        let mut buffer = String::new();
        match input.read_to_string(&mut buffer) {
            Ok(_) => Ok(CodeSource {
                language,
                filename: path.to_string_lossy().to_string(),
                buffer,
            }),
            Err(err) => Err(LogError::CannotReadSourceFile {
                path: PathBuf::from(path),
                source: err,
            }),
        }
    }

    pub fn ts_language(&self) -> Language {
        match self.language {
            SourceLanguage::Rust => tree_sitter_rust_orchard::LANGUAGE.into(),
            SourceLanguage::Java => tree_sitter_java::LANGUAGE.into(),
            SourceLanguage::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        }
    }

    pub fn find_code(path: &Path, filter: Option<Vec<String>>) -> (Vec<CodeSource>, Vec<LogError>) {
        let mut srcs = vec![];
        let mut errs = vec![];
        match fs::metadata(path) {
            Ok(meta) => {
                if meta.is_file() {
                    try_add_file(path, &mut srcs, &mut errs, &filter);
                } else {
                    if let Err(err) = walk_dir(path, &mut srcs, &mut errs, &filter) {
                        errs.push(err);
                    }
                }
            }
            Err(err) => errs.push(LogError::CannotReadSourceFile {
                path: PathBuf::from(path),
                source: err,
            }),
        }
        (srcs, errs)
    }
}

const SUPPORTED_EXTS: &[&str] = &["java", "rs", "h", "hh", "hpp", "cc", "cpp"];

fn try_add_file(
    path: &Path,
    srcs: &mut Vec<CodeSource>,
    errs: &mut Vec<LogError>,
    filter: &Option<Vec<String>>,
) {
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

    if let Some(ext) = path.extension() {
        if SUPPORTED_EXTS.iter().any(|&supported| supported == ext) {
            match File::open(path) {
                Ok(file) => {
                    let input = Box::new(file);
                    match CodeSource::new(path, input) {
                        Ok(code) => srcs.push(code),
                        Err(err) => errs.push(err),
                    }
                }
                Err(err) => errs.push(LogError::CannotReadSourceFile {
                    path: PathBuf::from(path),
                    source: err,
                })
            }
        }
    }
}

fn walk_dir(
    dir: &Path,
    srcs: &mut Vec<CodeSource>,
    errs: &mut Vec<LogError>,
    filter: &Option<Vec<String>>,
) -> Result<(), LogError> {
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|source| LogError::CannotAccessPath {
                    path: PathBuf::from(dir),
                    source,
                })?;
                let path = entry.path();
                let metadata = fs::metadata(&path).map_err(|source| LogError::CannotAccessPath {
                    path: PathBuf::from(dir),
                    source,
                })?;
                if metadata.is_file() {
                    try_add_file(&path, srcs, errs, filter);
                } else if metadata.is_dir() {
                    if let Err(err) = walk_dir(&path, srcs, errs, filter) {
                        errs.push(err);
                    }
                }
            }
        }
        Err(_) => {}
    }
    Ok(())
}
