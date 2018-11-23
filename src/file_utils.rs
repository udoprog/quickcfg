//! Thread-safe utilities for creating files and directories.
//! use std::collections::HashMap;
//!
use crate::unit::{CopyFile, CreateDir, Symlink, SystemUnit, UnitAllocator, UnitId};
use failure::{bail, format_err, Error};
use fxhash::FxHashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

struct FileUtilsInner {
    directories: FxHashMap<PathBuf, UnitId>,
    files: FxHashMap<PathBuf, UnitId>,
}

/// Utilities to build units for creating directories.
pub struct FileUtils<'a> {
    allocator: &'a UnitAllocator,
    state_dir: PathBuf,
    inner: RwLock<FileUtilsInner>,
}

impl<'a> FileUtils<'a> {
    /// Create new, thread-safe file utilities.
    pub fn new(state_dir: &Path, allocator: &'a UnitAllocator) -> FileUtils<'a> {
        FileUtils {
            allocator,
            state_dir: state_dir.to_owned(),
            inner: RwLock::new(FileUtilsInner {
                directories: FxHashMap::default(),
                files: FxHashMap::default(),
            }),
        }
    }

    /// Set up the unit to copy a file.
    pub fn symlink(&self, path: &Path, link: PathBuf) -> Result<SystemUnit, Error> {
        let mut inner = self
            .inner
            .write()
            .map_err(|_| format_err!("lock poisoned"))?;

        let mut unit = self.allocator.unit(Symlink {
            path: path.to_owned(),
            link,
        });

        if let Some(parent) = path.parent() {
            unit.dependencies
                .extend(inner.directories.get(parent).cloned());
        }

        if let Some(_) = inner.files.insert(path.to_owned(), unit.id) {
            bail!("Multiple systems try to modify file: {}", path.display());
        }

        Ok(unit)
    }

    /// Set up the unit to copy a file.
    pub fn copy_file(&self, from: &Path, to: &Path, templates: bool) -> Result<SystemUnit, Error> {
        let mut inner = self
            .inner
            .write()
            .map_err(|_| format_err!("lock poisoned"))?;

        let mut unit = self.allocator.unit(CopyFile {
            from: from.to_owned(),
            to: to.to_owned(),
            templates,
        });

        if let Some(parent) = to.parent() {
            unit.dependencies
                .extend(inner.directories.get(parent).cloned());
        }

        if let Some(_) = inner.files.insert(to.to_owned(), unit.id) {
            bail!("Multiple systems try to modify file: {}", to.display());
        }

        Ok(unit)
    }

    /// Recursively set up units with dependencies to create the given directories.
    pub fn create_dir_all(&self, dir: &Path) -> Result<Vec<SystemUnit>, Error> {
        let dirs = {
            let inner = self
                .inner
                .read()
                .map_err(|_| format_err!("lock poisoned"))?;

            // Directory is already being created.
            if inner.directories.contains_key(dir) {
                return Ok(vec![]);
            }

            if dir.is_dir() {
                return Ok(vec![]);
            }

            let mut dirs = Vec::new();

            let mut c = dir;
            dirs.push(c.clone());

            // Build up collection to create until we have found what we wanted.
            while let Some(parent) = c.parent() {
                if parent.is_dir() {
                    break;
                }

                if inner.directories.contains_key(parent) {
                    break;
                }

                dirs.push(parent);
                c = parent;
            }

            dirs
        };

        let mut inner = self
            .inner
            .write()
            .map_err(|_| format_err!("lock poisoned"))?;

        let mut out = Vec::new();

        for dir in dirs.into_iter().rev() {
            // needs to re-check now that we have mutable access.
            if inner.directories.contains_key(dir) {
                continue;
            }

            let mut unit = self.allocator.unit(CreateDir(dir.to_owned()));
            inner.directories.insert(dir.to_owned(), unit.id);

            if let Some(parent) = dir.parent() {
                unit.dependencies
                    .extend(inner.directories.get(parent).cloned());
            }

            out.push(unit);
        }

        Ok(out)
    }

    /// Get the state path for the given ID.
    pub fn state_path(&self, id: &str) -> PathBuf {
        self.state_dir.join(id)
    }

    /// Try to open metadata, unless the file does not exist.
    ///
    /// If the file does not exist, returns `None`.
    pub fn try_open_meta(p: &Path) -> Result<Option<fs::Metadata>, Error> {
        match p.symlink_metadata() {
            Ok(m) => Ok(Some(m)),
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => Ok(None),
                _ => bail!("to get metadata: {}: {}", p.display(), e),
            },
        }
    }

    /// Test if we should create the specified symlink.
    pub fn should_create_symlink(
        path: &Path,
        link: &Path,
        meta: Option<&fs::Metadata>,
    ) -> Result<bool, Error> {
        let meta = match meta {
            Some(meta) => meta,
            None => return Ok(true),
        };

        let ty = meta.file_type();

        if !ty.is_symlink() {
            bail!("File exists but is not a symlink: {}", path.display());
        }

        let actual_link = fs::read_link(path)?;

        if actual_link != link {
            bail!(
                "Symlink exists `{}`, but contains the wrong link `{}`, expected: {}",
                path.display(),
                actual_link.display(),
                link.display(),
            );
        }

        Ok(false)
    }
}
