//! Thread-safe utilities for creating files and directories.
//! use std::collections::HashMap;
//!
use crate::{
    hierarchy::Data,
    opts::Opts,
    unit::{CopyFile, CreateDir, Dependency, Symlink, SystemUnit, UnitAllocator, UnitId},
};
use failure::{bail, format_err, Error, ResultExt};
use fxhash::FxHashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::SystemTime;

/// Utilities to build units for creating directories.
pub struct FileUtils<'a> {
    opts: &'a Opts,
    state_dir: PathBuf,
    allocator: &'a UnitAllocator,
    data: &'a Data,
    directories: RwLock<FxHashMap<PathBuf, UnitId>>,
    files: RwLock<FxHashMap<PathBuf, UnitId>>,
}

impl<'a> FileUtils<'a> {
    /// Create new, thread-safe file utilities.
    pub fn new(
        opts: &'a Opts,
        state_dir: &Path,
        allocator: &'a UnitAllocator,
        data: &'a Data,
    ) -> FileUtils<'a> {
        FileUtils {
            opts,
            state_dir: state_dir.to_owned(),
            allocator,
            data,
            directories: RwLock::new(FxHashMap::default()),
            files: RwLock::new(FxHashMap::default()),
        }
    }

    /// Access or allocate a file dependency of the given path.
    pub fn file_dependency(&self, path: &Path) -> Result<Dependency, Error> {
        self.get_or_insert(&self.files, path).map(Dependency::File)
    }

    /// Access or allocate a directory dependency of the given path.
    pub fn dir_dependency(&self, path: &Path) -> Result<Dependency, Error> {
        self.get_or_insert(&self.directories, path)
            .map(Dependency::Dir)
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

        if let Some(parent) = path.parent() {
            if !parent.is_dir() {
                unit.dependencies.push(self.dir_dependency(parent)?);
            }
        }

        unit.provides.push(self.file_dependency(path)?);
        Ok(Some(unit))
    }

    /// Optionally set up if we should copy a file.
    ///
    /// This is true if:
    ///
    /// * The destination file does not exist.
    /// * The destination file has a modified timestamp less than the source file.
    pub fn copy_file(&self, from: &Path, to: &Path, templates: bool) -> Result<SystemUnit, Error> {
        let mut unit = self.allocator.unit(CopyFile {
            from: from.to_owned(),
            to: to.to_owned(),
            templates,
        });

        if let Some(parent) = to.parent() {
            if !parent.is_dir() {
                unit.dependencies.push(self.dir_dependency(parent)?);
            }
        }

        unit.provides.push(self.file_dependency(to)?);
        Ok(unit)
    }

    /// Recursively set up units with dependencies to create the given directories.
    pub fn create_dir_all(&self, dir: &Path) -> Result<Vec<SystemUnit>, Error> {
        let dirs = {
            let directories = self
                .directories
                .read()
                .map_err(|_| format_err!("lock poisoned"))?;

            // Directory is already being created.
            if directories.contains_key(dir) {
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

                if directories.contains_key(parent) {
                    break;
                }

                dirs.push(parent);
                c = parent;
            }

            dirs
        };

        let mut directories = self
            .directories
            .write()
            .map_err(|_| format_err!("lock poisoned"))?;

        let mut out = Vec::new();

        for dir in dirs.into_iter().rev() {
            // needs to re-check now that we have mutable access.
            if directories.contains_key(dir) {
                continue;
            }

            let mut unit = self.allocator.unit(CreateDir(dir.to_owned()));
            directories.insert(dir.to_owned(), unit.id);
            unit.provides.push(Dependency::Dir(unit.id));

            if let Some(parent) = dir.parent() {
                unit.dependencies
                    .extend(directories.get(parent).cloned().map(Dependency::Dir));
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
        &self,
        from: &fs::Metadata,
        to_path: &Path,
        to: Option<&fs::Metadata>,
        template: bool,
    ) -> Result<bool, Error> {
        let to = match to {
            Some(to) => to,
            None => return Ok(true),
        };

        if !to.is_file() {
            bail!("Exists but is not a file: {}", to_path.display());
        }

        let to_modified = to.modified()?;

        if from.modified()? > to_modified {
            return Ok(true);
        }

        // if a template, we want to check if hierarchy was modified.
        if template {
            match self.data.last_modified.as_ref() {
                Some(data_modified) if *data_modified > to_modified => return Ok(true),
                _ => {}
            }
        }

        Ok(false)
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

    /// Update timestamps for the argument `to`, based on `from`.
    pub fn update_timestamps(now: &SystemTime, path: &Path) -> Result<(), Error> {
        use filetime::{self, FileTime};

        let m_time = FileTime::from_system_time(now.clone());
        let a_time = m_time.clone();

        filetime::set_file_times(path, a_time, m_time)
            .with_context(|_| format_err!("Failed to update timestamps for: {}", path.display()))?;
        return Ok(());
    }

    #[inline]
    fn get_or_insert<K: ?Sized>(
        &self,
        map: &RwLock<FxHashMap<K::Owned, UnitId>>,
        k: &K,
    ) -> Result<UnitId, Error>
    where
        K: std::hash::Hash + Eq + ToOwned,
        K::Owned: std::hash::Hash + Eq,
    {
        {
            let m = map.read().map_err(|_| format_err!("lock poisoned"))?;

            if let Some(id) = m.get(k).cloned() {
                return Ok(id);
            }
        }

        let mut m = map.write().map_err(|_| format_err!("lock poisoned"))?;

        let id = self.allocator.allocate();
        m.insert(k.to_owned(), id);
        Ok(id)
    }
}
