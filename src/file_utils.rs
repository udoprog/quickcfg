//! Thread-safe utilities for creating files and directories.
//! use std::collections::HashMap;
//!
use crate::unit::{CopyFile, CreateDir, SystemUnit, UnitAllocator, UnitId};
use failure::{bail, format_err, Error};
use fxhash::FxHashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

struct FileUtilsInner {
    directories: FxHashMap<PathBuf, UnitId>,
    files: FxHashMap<PathBuf, UnitId>,
}

/// Utilities to build units for creating directories.
pub struct FileUtils<'a> {
    allocator: &'a UnitAllocator,
    inner: RwLock<FileUtilsInner>,
}

impl<'a> FileUtils<'a> {
    /// Create new, thread-safe file utilities.
    pub fn new(allocator: &'a UnitAllocator) -> FileUtils<'a> {
        FileUtils {
            allocator,
            inner: RwLock::new(FileUtilsInner {
                directories: FxHashMap::default(),
                files: FxHashMap::default(),
            }),
        }
    }

    /// Set up the unit to copy a file.
    pub fn copy_file(&self, from: &Path, to: &Path) -> Result<SystemUnit, Error> {
        let mut inner = self
            .inner
            .write()
            .map_err(|_| format_err!("lock poisoned"))?;

        let mut unit = self
            .allocator
            .unit(CopyFile(from.to_owned(), to.to_owned()));

        if let Some(parent) = to.parent() {
            unit.add_dependencies(inner.directories.get(parent).cloned());
        }

        if let Some(_) = inner.files.insert(to.to_owned(), unit.id) {
            bail!("multiple systems try to modify file: {}", to.display());
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
                unit.add_dependencies(inner.directories.get(parent).cloned());
            }

            out.push(unit);
        }

        Ok(out)
    }
}
