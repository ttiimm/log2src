use crate::{LogError, SourceLanguage};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::{fs, io};

/// Result of a shallow check of a file system path.  Mainly interested in getting a directory
/// listing without descending into the child trees.
enum ShallowCheckResult {
    File {
        latest_modified_time: SystemTime,
    },
    Directory {
        latest_entries: BTreeMap<OsString, Result<fs::Metadata, io::Error>>,
    },
    Error,
}

/// A unique identifier for a file that can be used instead of retaining the full path.
#[derive(Copy, Clone, Debug, Serialize, Hash, Eq, PartialEq)]
pub struct SourceFileID(usize);

/// A summary of a source code file
#[derive(Copy, Clone, Debug, Serialize)]
pub struct SourceFileInfo {
    pub language: SourceLanguage,
    pub id: SourceFileID,
}

impl SourceFileInfo {
    // Allow the ID counter to be set for a thread from the SourceHierTree.
    thread_local! {
        static NEXT_ID: RefCell<usize> = RefCell::new(0);
    }

    pub fn new(language: SourceLanguage) -> Self {
        Self::NEXT_ID.with(|next_id| {
            let mut inner = next_id.borrow_mut();
            let id = SourceFileID(*inner);
            *inner += 1;
            Self { language, id }
        })
    }
}

/// The type of content in a node in the source hierarchy
#[derive(Debug)]
pub enum SourceHierContent {
    File {
        info: SourceFileInfo,
        last_modified_time: SystemTime,
    },
    UnsupportedFile {},
    Directory {
        entries: BTreeMap<OsString, SourceHierNode>,
    },
    Error {
        source: LogError,
    },
    Unknown {},
}

impl SourceHierContent {
    fn entries_of(
        path: &Path,
    ) -> Result<BTreeMap<OsString, Result<fs::Metadata, io::Error>>, io::Error> {
        Ok(fs::read_dir(path)?
            .flat_map(|entry| match entry {
                Ok(entry) => Some((entry.file_name(), entry.metadata())),
                Err(_err) => None,
            })
            .collect())
    }

    fn from_dir(path: &Path) -> Self {
        match Self::entries_of(path) {
            Ok(entries) => Self::Directory {
                entries: entries
                    .into_iter()
                    .map(|(entry_name, meta)| {
                        (
                            entry_name.to_os_string(),
                            SourceHierNode::from_int(&path.join(entry_name), meta),
                        )
                    })
                    .collect(),
            },
            Err(err) => Self::Error {
                source: LogError::CannotAccessPath {
                    path: path.to_path_buf(),
                    source: err.into(),
                },
            },
        }
    }

    fn from(path: &Path, metadata: Result<fs::Metadata, io::Error>) -> Self {
        match metadata {
            Ok(meta) => {
                if meta.is_dir() {
                    Self::from_dir(path)
                } else if meta.is_file() {
                    match SourceLanguage::from_path(&path) {
                        Some(language) => match meta.modified() {
                            Ok(last_modified_time) => Self::File {
                                info: SourceFileInfo::new(language),
                                last_modified_time,
                            },
                            Err(err) => Self::Error {
                                source: LogError::CannotAccessPath {
                                    path: path.to_path_buf(),
                                    source: err.into(),
                                },
                            },
                        },
                        None => Self::UnsupportedFile {},
                    }
                } else {
                    Self::Unknown {}
                }
            }
            Err(err) => Self::Error {
                source: LogError::CannotAccessPath {
                    path: path.to_path_buf(),
                    source: err.into(),
                },
            },
        }
    }

    fn shallow_check(
        path: &Path,
        metadata: &Result<fs::Metadata, io::Error>,
    ) -> ShallowCheckResult {
        match metadata {
            Ok(meta) => {
                if meta.is_file() {
                    match meta.modified() {
                        Ok(latest_modified_time) => ShallowCheckResult::File {
                            latest_modified_time,
                        },
                        Err(_) => ShallowCheckResult::Error,
                    }
                } else if meta.is_dir() {
                    match Self::entries_of(path) {
                        Ok(latest_entries) => ShallowCheckResult::Directory { latest_entries },
                        Err(_) => ShallowCheckResult::Error,
                    }
                } else {
                    ShallowCheckResult::Error
                }
            }
            Err(_) => ShallowCheckResult::Error,
        }
    }

    /// Synchronized this content with the current state on the file system.
    ///
    /// # Cases
    /// ## Files
    /// If a file exists and has the same modified time as the last sync, nothing is done.
    /// Otherwise, a new content value is created from the file system state and self is
    /// overwritten with that value.
    ///
    /// ## Directories
    /// A shallow scan of the directory is done to gather file names and metadata.  If files in
    /// the current state are not found in the shallow scan, ScanEvent::DeletedFile events will
    /// be added to the "deleted_events" vector.  If files in the current state are in the
    /// shallow state, they will be synced individually.  If there are files in the shallow scan
    /// that are not in the current state, they will be instantiated added as children of the
    /// directory.
    fn sync_int(
        &mut self,
        path: &Path,
        latest_meta: Result<fs::Metadata, io::Error>,
        deleted_events: &mut Vec<ScanEvent>,
    ) -> bool {
        let latest_content = Self::shallow_check(path, &latest_meta);
        *self = match self {
            SourceHierContent::File {
                last_modified_time,
                info,
                ..
            } => match latest_content {
                ShallowCheckResult::File {
                    latest_modified_time,
                    ..
                } if *last_modified_time == latest_modified_time => {
                    return false;
                }
                _ => {
                    deleted_events.push(ScanEvent::DeletedFile(PathBuf::from(path), info.id));
                    Self::from(path, latest_meta)
                }
            },
            SourceHierContent::Directory { ref mut entries } => match latest_content {
                ShallowCheckResult::Directory { latest_entries } => {
                    let mut changed = false;
                    entries.retain(|name, node| {
                        let exists = latest_entries.contains_key(name);
                        if !exists {
                            node.deleted(path, name, deleted_events);
                            changed = true;
                        }
                        exists
                    });
                    let mut new_entries: Vec<(OsString, Result<fs::Metadata, io::Error>)> =
                        Vec::new();
                    for (name, meta) in latest_entries {
                        if let Some(existing_entry) = entries.get_mut(&name) {
                            existing_entry.sync(&path.join(&name), meta, deleted_events)
                        } else {
                            new_entries.push((name, meta));
                            changed = true;
                        }
                    }
                    new_entries.into_iter().for_each(|(name, meta)| {
                        let node = SourceHierNode::from_int(&path.join(&name), meta);
                        entries.insert(name, node);
                    });
                    return changed;
                }
                _ => Self::from(path, latest_meta),
            },
            _ => Self::from(path, latest_meta),
        };
        true
    }
}

/// A node in the SourceHierTree.  It contains information that is common to all types of content
/// and the content itself (e.g. file, directory, error, ...).
#[derive(Debug)]
pub struct SourceHierNode {
    pub last_scan_time: Option<SystemTime>,
    pub content: SourceHierContent,
}

impl SourceHierNode {
    fn from_int(path: &Path, metadata: Result<fs::Metadata, io::Error>) -> Self {
        match metadata {
            Ok(meta) => {
                if meta.is_dir() {
                    Self {
                        last_scan_time: None,
                        content: SourceHierContent::from_dir(path),
                    }
                } else if meta.is_file() {
                    match SourceLanguage::from_path(&path) {
                        Some(language) => match meta.modified() {
                            Ok(last_modified_time) => Self {
                                last_scan_time: None,
                                content: SourceHierContent::File {
                                    info: SourceFileInfo::new(language),
                                    last_modified_time,
                                },
                            },
                            Err(err) => Self {
                                last_scan_time: None,
                                content: SourceHierContent::Error {
                                    source: LogError::CannotAccessPath {
                                        path: path.to_path_buf(),
                                        source: err.into(),
                                    },
                                },
                            },
                        },
                        None => Self {
                            last_scan_time: None,
                            content: SourceHierContent::UnsupportedFile {},
                        },
                    }
                } else {
                    Self {
                        last_scan_time: None,
                        content: SourceHierContent::Unknown {},
                    }
                }
            }
            Err(err) => Self {
                last_scan_time: None,
                content: SourceHierContent::Error {
                    source: LogError::CannotAccessPath {
                        path: path.to_path_buf(),
                        source: err.into(),
                    },
                },
            },
        }
    }

    /// Create a node with unknown content that will be synced at a later point.
    fn stub() -> Self {
        SourceHierNode {
            last_scan_time: None,
            content: SourceHierContent::Unknown {},
        }
    }

    fn deleted(&self, path: &Path, name: &OsStr, deleted_events: &mut Vec<ScanEvent>) {
        match &self.content {
            SourceHierContent::File { info, .. } => {
                deleted_events.push(ScanEvent::DeletedFile(path.join(name), info.id))
            }
            SourceHierContent::UnsupportedFile { .. } => {}
            SourceHierContent::Directory { entries } => {
                let dir_path = path.join(name);
                for (child_name, node) in entries {
                    node.deleted(&dir_path, &child_name, deleted_events);
                }
            }
            SourceHierContent::Error { .. } => {}
            SourceHierContent::Unknown { .. } => {}
        }
    }

    fn sync(
        &mut self,
        path: &Path,
        meta: Result<fs::Metadata, io::Error>,
        deleted_events: &mut Vec<ScanEvent>,
    ) {
        if self.content.sync_int(path, meta, deleted_events) {
            self.last_scan_time = None;
        }
    }
}

/// An event when iterating over the value returned by the [`scan()`](SourceHierTree::scan())
/// method.
#[derive(Debug, Serialize)]
pub enum ScanEvent {
    NewFile(PathBuf, SourceFileInfo),
    DeletedFile(PathBuf, SourceFileID),
}

struct TreeCursorMut<'a> {
    curr_path: PathBuf,
    curr_node: &'a mut SourceHierNode,
}

pub struct TreeScanner<'a> {
    deleted_events: Vec<ScanEvent>,
    stack: Vec<TreeCursorMut<'a>>,
}

impl Iterator for TreeScanner<'_> {
    type Item = ScanEvent;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(event) = self.deleted_events.pop() {
            return Some(event);
        }
        while let Some(cursor) = self.stack.pop() {
            let last_scan_time = cursor.curr_node.last_scan_time;
            cursor.curr_node.last_scan_time = Some(SystemTime::now());
            match &mut cursor.curr_node.content {
                SourceHierContent::File { info, .. } => match last_scan_time {
                    Some(_) => {}
                    _ => return Some(ScanEvent::NewFile(cursor.curr_path, *info)),
                },
                SourceHierContent::UnsupportedFile { .. } => {}
                SourceHierContent::Directory { ref mut entries } => {
                    for child in entries.iter_mut() {
                        self.stack.push(TreeCursorMut {
                            curr_path: cursor.curr_path.join(&child.0),
                            curr_node: child.1,
                        });
                    }
                }
                SourceHierContent::Error { .. } => {}
                SourceHierContent::Unknown {} => {}
            }
        }
        None
    }
}

#[derive(Debug, Serialize, Default)]
pub struct SourceHierStats {
    pub files: usize,
    pub unsupported_files: usize,
    pub directories: usize,
    pub errors: usize,
}

/// A SourceHierTree tracks the state of a source code hierarchy.
#[derive(Debug)]
pub struct SourceHierTree {
    pub root_path: PathBuf,
    pub root_node: SourceHierNode,
    next_id: usize,
    deleted_events: Vec<ScanEvent>,
}

impl SourceHierTree {
    pub fn from(path: &Path) -> SourceHierTree {
        SourceHierTree {
            root_path: path.to_path_buf(),
            root_node: SourceHierNode::stub(),
            next_id: 0,
            deleted_events: Vec::new(),
        }
    }

    /// Synchronize the state of this tree with the file system.
    pub fn sync(&mut self) {
        SourceFileInfo::NEXT_ID.with(|id_opt| {
            *id_opt.borrow_mut() = self.next_id;
        });
        self.root_node.sync(
            &self.root_path,
            fs::metadata(&self.root_path),
            &mut self.deleted_events,
        );
        self.next_id = SourceFileInfo::NEXT_ID.with(|id_opt| *id_opt.borrow());
    }

    /// Scan the tree for changes that have happened since the last scan.  Changes to the tree
    /// are introduced by the sync() method.
    pub fn scan(&'_ mut self) -> TreeScanner<'_> {
        let deleted_events = std::mem::replace(&mut self.deleted_events, Vec::new());
        TreeScanner {
            deleted_events,
            stack: vec![TreeCursorMut {
                curr_path: self.root_path.clone(),
                curr_node: &mut self.root_node,
            }],
        }
    }

    /// Visit every node in the hierarchy, depth-first, calling `f` on each.
    pub fn visit<F>(&self, mut f: F)
    where
        F: FnMut(&SourceHierNode),
    {
        fn walk<F>(node: &SourceHierNode, f: &mut F)
        where
            F: FnMut(&SourceHierNode),
        {
            f(node);
            if let SourceHierContent::Directory { entries } = &node.content {
                for child in entries.values() {
                    walk(child, f);
                }
            }
        }
        walk(&self.root_node, &mut f);
    }

    pub fn stats(&self) -> SourceHierStats {
        let mut retval = SourceHierStats::default();

        self.visit(|node| match node.content {
            SourceHierContent::File { .. } => retval.files += 1,
            SourceHierContent::UnsupportedFile { .. } => retval.unsupported_files += 1,
            SourceHierContent::Directory { .. } => retval.directories += 1,
            SourceHierContent::Error { .. } => retval.errors += 1,
            SourceHierContent::Unknown { .. } => {}
        });

        retval
    }
}

#[cfg(test)]
mod test {
    use crate::source_hier::{ScanEvent, SourceHierTree};
    use fs_extra::dir::copy;
    use fs_extra::dir::CopyOptions;
    use insta::assert_yaml_snapshot;
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::ops::Sub;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};
    use tempfile::{tempdir, TempDir};

    fn setup_test_environment(source_dir: &Path) -> TempDir {
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let dest_path = temp_dir.path().to_path_buf();

        // Copy the source directory content to the temporary directory
        let mut options = CopyOptions::new();
        options.overwrite = true; // Overwrite if files exist
        options.copy_inside = true; // Copy contents of source_dir into dest_path

        copy(source_dir, &dest_path, &options)
            .expect("Failed to copy source directory to temporary directory");

        temp_dir
    }

    fn redact_event(event: ScanEvent) -> ScanEvent {
        match event {
            ScanEvent::NewFile(path, info) => {
                ScanEvent::NewFile(path.file_name().unwrap().into(), info)
            }
            ScanEvent::DeletedFile(path, id) => {
                ScanEvent::DeletedFile(path.file_name().unwrap().into(), id)
            }
        }
    }

    #[test]
    fn test_with_resources_dir() {
        let tests_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
        let temp_test_dir = setup_test_environment(&tests_path);
        {
            let basic_file = File::open(temp_test_dir.path().join("tests/java/Basic.java"))
                .unwrap();
            let metadata = basic_file.metadata().unwrap();
            let mut perms = metadata.permissions();
            perms.set_readonly(false);
            basic_file.set_permissions(perms).unwrap();
            basic_file.set_modified(SystemTime::now().sub(Duration::from_secs(10)))
            .unwrap();
        }
        let mut tree = SourceHierTree::from(temp_test_dir.path());
        tree.sync();
        let events: Vec<ScanEvent> = tree.scan().map(redact_event).collect();
        assert_yaml_snapshot!(events);
        let no_events: Vec<ScanEvent> = tree.scan().map(redact_event).collect();
        assert_yaml_snapshot!(no_events);
        let _ = fs::remove_file(temp_test_dir.path().join("tests/test_java.rs")).unwrap();
        let _ = File::create(temp_test_dir.path().join("new.rs"))
            .unwrap()
            .write("abc".as_bytes())
            .unwrap();
        let _ = File::options()
            .append(true)
            .open(temp_test_dir.path().join("tests/java/Basic.java"))
            .unwrap()
            .write("def".as_bytes())
            .unwrap();
        tree.sync();
        let new_and_updated_events: Vec<ScanEvent> = tree.scan().map(redact_event).collect();
        assert_yaml_snapshot!(new_and_updated_events);
        let _ = fs::remove_dir_all(temp_test_dir.path().join("tests/java")).unwrap();
        tree.sync();
        let deleted_dir_events: Vec<ScanEvent> = tree.scan().map(redact_event).collect();
        assert_yaml_snapshot!(deleted_dir_events);
    }
}
