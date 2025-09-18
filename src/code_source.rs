use std::io;
use std::path::{Path, PathBuf};

use crate::source_hier::SourceFileInfo;
use crate::{LogError, SourceLanguage};

pub struct CodeSource {
    pub(crate) filename: String,
    pub(crate) info: SourceFileInfo,
    pub(crate) buffer: String,
}

impl CodeSource {
    pub fn new<I>(path: &Path, info: SourceFileInfo, mut input: I) -> Result<CodeSource, LogError>
    where
        I: io::Read,
    {
        let mut buffer = String::new();
        match input.read_to_string(&mut buffer) {
            Ok(_) => Ok(CodeSource {
                filename: path.to_string_lossy().to_string(),
                info,
                buffer,
            }),
            Err(err) => Err(LogError::CannotReadSourceFile {
                path: PathBuf::from(path),
                source: err.into(),
            }),
        }
    }

    pub fn from_string(path: &Path, input: &str) -> CodeSource {
        CodeSource {
            filename: path.to_string_lossy().to_string(),
            info: SourceFileInfo::new(SourceLanguage::from_path(path).unwrap()),
            buffer: input.to_string(),
        }
    }
}
