//! Thread-safe utilities for creating files and directories.
//! use std::collections::HashMap;
//!
use crate::{
    opts::Opts,
    unit::{CopyFile, CreateDir, Symlink, SystemUnit, UnitAllocator, UnitId},
};
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
    opts: &'a Opts,
    state_dir: PathBuf,
    allocator: &'a UnitAllocator,
    inner: RwLock<FileUtilsInner>,
}

impl<'a> FileUtils<'a> {
    /// Create new, thread-safe file utilities.
    pub fn new(opts: &'a Opts, state_dir: &Path, allocator: &'a UnitAllocator) -> FileUtils<'a> {
        FileUtils {
            opts,
            allocator,
            state_dir: state_dir.to_owned(),
            inner: RwLock::new(FileUtilsInner {
                directories: FxHashMap::default(),
                files: FxHashMap::default(),
            }),
        }
    }

    /// Indicate that a file is being modified by a unit.
    pub fn insert_file(&self, path: &Path, unit: &SystemUnit) -> Result<(), Error> {
        let mut inner = self
            .inner
            .write()
            .map_err(|_| format_err!("lock poisoned"))?;

        if let Some(_) = inner.files.insert(path.to_owned(), unit.id) {
            bail!("Multiple systems try to modify file: {}", path.display());
        }

        Ok(())
    }

    /// Indicate that a directory is being modified by a unit.
    pub fn insert_directory(&self, path: &Path, unit: &SystemUnit) -> Result<(), Error> {
        let mut inner = self
            .inner
            .write()
            .map_err(|_| format_err!("lock poisoned"))?;

        if let Some(_) = inner.directories.insert(path.to_owned(), unit.id) {
            bail!(
                "Multiple systems try to modify directory: {}",
                path.display()
            );
        }

        Ok(())
    }

    /// Try to create a symlink.
    pub fn symlink(
        &self,
        path: &Path,
        link: PathBuf,
        meta: Option<&fs::Metadata>,
    ) -> Result<Option<SystemUnit>, Error> {
        let remove = match meta {
            Some(meta) => {
                let ty = meta.file_type();

                if !ty.is_symlink() {
                    bail!("File exists but is not a symlink: {}", path.display());
                }

                let actual_link = fs::read_link(path)?;

                if actual_link == link {
                    return Ok(None);
                }

                if !self.opts.force {
                    bail!(
                        "Symlink exists `{}`, but contains the wrong link `{}`, expected: {} (use `--force` to override)",
                        path.display(),
                        actual_link.display(),
                        link.display(),
                    );
                }

                true
            }
            None => false,
        };

        let mut unit = self.allocator.unit(Symlink {
            remove,
            path: path.to_owned(),
            link,
        });

        let mut inner = self
            .inner
            .write()
            .map_err(|_| format_err!("lock poisoned"))?;

        if let Some(parent) = path.parent() {
            unit.dependencies
                .extend(inner.directories.get(parent).cloned());
        }

        if let Some(_) = inner.files.insert(path.to_owned(), unit.id) {
            bail!("Multiple systems trying to modify file: {}", path.display());
        }

        Ok(Some(unit))
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

    /// Test if we should create the destination directory.
    pub fn should_create_dir(path: &Path, meta: Option<&fs::Metadata>) -> Result<bool, Error> {
        let meta = match meta {
            Some(meta) => meta,
            None => return Ok(true),
        };

        let ty = meta.file_type();

        if !ty.is_dir() {
            bail!("Exists but is not a dir: {}", path.display());
        }

        Ok(false)
    }

    /// Test if we should copy the file.
    ///
    /// This is true if:
    ///
    /// * The destination file does not exist.
    /// * The destination file has a modified timestamp less than the source file.
    pub fn should_copy_file(
        from: &fs::Metadata,
        to_path: &Path,
        to: Option<&fs::Metadata>,
    ) -> Result<bool, Error> {
        let to = match to {
            Some(to) => to,
            None => return Ok(true),
        };

        if !to.is_file() {
            bail!("Exists but is not a file: {}", to_path.display());
        }

        Ok(from.modified()? > to.modified()?)
    }

    /// Construct a relative path from a provided base directory path to the provided path
    ///
    /// ```rust
    /// use quickcfg::FileUtils;
    /// use std::path::PathBuf;
    ///
    /// let baz: PathBuf = "/foo/bar/baz".into();
    /// let bar: PathBuf = "/foo/bar".into();
    /// let quux: PathBuf = "/foo/bar/quux".into();
    /// assert_eq!(FileUtils::path_relative_from(&bar, &baz), Some("../".into()));
    /// assert_eq!(FileUtils::path_relative_from(&baz, &bar), Some("baz".into()));
    /// assert_eq!(FileUtils::path_relative_from(&quux, &baz), Some("../quux".into()));
    /// assert_eq!(FileUtils::path_relative_from(&baz, &quux), Some("../baz".into()));
    /// assert_eq!(FileUtils::path_relative_from(&bar, &quux), Some("../".into()));
    ///
    /// ```
    pub fn path_relative_from(path: &Path, base: &Path) -> Option<PathBuf> {
        // Adapted from:
        // https://github.com/Manishearth/pathdiff/blob/f64de9f529424c43fe07cd5f16f4160c6fdab224/src/lib.rs
        use std::path::Component;

        if path.is_absolute() != base.is_absolute() {
            if path.is_absolute() {
                return Some(PathBuf::from(path));
            } else {
                return None;
            }
        }

        let mut ita = path.components();
        let mut itb = base.components();

        let mut comps: Vec<Component> = vec![];

        loop {
            match (ita.next(), itb.next()) {
                (None, None) => break,
                (Some(a), None) => {
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
                (None, _) => comps.push(Component::ParentDir),
                (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                (Some(a), Some(b)) if b == Component::CurDir => comps.push(a),
                (Some(_), Some(b)) if b == Component::ParentDir => return None,
                (Some(a), Some(_)) => {
                    comps.push(Component::ParentDir);
                    for _ in itb {
                        comps.push(Component::ParentDir);
                    }
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
            }
        }

        Some(comps.iter().map(|c| c.as_os_str()).collect())
    }
}
