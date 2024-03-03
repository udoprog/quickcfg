//! Thread-safe utilities for creating files and directories.
//! use std::collections::HashMap;
//!
use crate::{
    hierarchy::Data,
    opts::Opts,
    unit::{CopyFile, CopyTemplate, CreateDir, Dependency, Symlink, SystemUnit, UnitAllocator},
};
use anyhow::{anyhow, bail, Context as _, Error};
use fxhash::FxHashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// Synchronized bits of FileSystem.
#[derive(Default)]
pub struct FileSystemInner {
    // TODO: include the system that modified the paths for better diagnostics.
    paths: FxHashMap<PathBuf, Dependency>,
    invalid: bool,
}

/// Helper and tracker of any filesystem modifications.
pub struct FileSystem<'a> {
    opts: &'a Opts,
    state_dir: PathBuf,
    allocator: &'a UnitAllocator,
    data: &'a Data,
    inner: Mutex<FileSystemInner>,
}

macro_rules! dependency {
    ($name:ident, $slf:ident, $path:ident) => {{
        let mut inner = $slf.inner.lock().map_err(|_| anyhow!("Lock poisoned"))?;

        if let Some(dependency) = inner.paths.get_mut($path) {
            if let Dependency::$name(_) = *dependency {
                return Ok(*dependency);
            }

            bail!(
                "Multiple systems modifying path `{}` in different ways",
                $path.display()
            );
        }

        let dependency = Dependency::$name($slf.allocator.allocate());
        Ok(*inner.paths.entry($path.to_owned()).or_insert(dependency))
    }};
}

impl<'a> FileSystem<'a> {
    /// Create new, thread-safe file utilities.
    pub fn new(
        opts: &'a Opts,
        state_dir: &Path,
        allocator: &'a UnitAllocator,
        data: &'a Data,
    ) -> FileSystem<'a> {
        FileSystem {
            opts,
            state_dir: state_dir.to_owned(),
            allocator,
            data,
            inner: Mutex::new(FileSystemInner::default()),
        }
    }

    /// Validate that we haven't created any conflicting files.
    /// Logs details and errors in case duplicates are registered.
    pub fn validate(self) -> Result<(), Error> {
        let inner = self.inner.lock().map_err(|_| anyhow!("Lock poisoned"))?;

        if !inner.invalid {
            return Ok(());
        }

        bail!("Multiple systems with conflicting path modifications");
    }

    /// Access or allocate a file dependency of the given path.
    pub fn file_dependency(&self, path: &Path) -> Result<Dependency, Error> {
        dependency!(File, self, path)
    }

    /// Access or allocate a directory dependency of the given path.
    pub fn dir_dependency(&self, path: &Path) -> Result<Dependency, Error> {
        dependency!(Dir, self, path)
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
    pub fn copy_file(
        &self,
        from: &Path,
        from_meta: fs::Metadata,
        to: &Path,
        to_meta: Option<&fs::Metadata>,
        template: bool,
    ) -> Result<Option<SystemUnit>, Error> {
        let from_modified = match self.should_copy_file(&from_meta, to, to_meta, template)? {
            Some(modified) => modified,
            None => return Ok(None),
        };

        let mut unit = if template {
            self.allocator.unit(CopyTemplate {
                from: from.to_owned(),
                from_modified,
                to: to.to_owned(),
                to_exists: to_meta.is_some(),
            })
        } else {
            self.allocator.unit(CopyFile {
                from: from.to_owned(),
                from_modified,
                to: to.to_owned(),
            })
        };

        if let Some(parent) = to.parent() {
            if !parent.is_dir() {
                unit.dependencies.push(self.dir_dependency(parent)?);
            }
        }

        unit.provides.push(self.file_dependency(to)?);
        Ok(Some(unit))
    }

    /// Recursively set up units with dependencies to create the given directories.
    pub fn create_dir_all(&self, dir: &Path) -> Result<Vec<SystemUnit>, Error> {
        let mut inner = self.inner.lock().map_err(|_| anyhow!("Lock poisoned"))?;

        let dirs = {
            // Directory is already being created.
            if inner.paths.contains_key(dir) {
                return Ok(vec![]);
            }

            if dir.is_dir() {
                return Ok(vec![]);
            }

            let mut dirs = Vec::new();

            let mut c = dir;
            dirs.push(c);

            // Build up collection to create until we have found what we wanted.
            while let Some(parent) = c.parent() {
                if parent.is_dir() {
                    break;
                }

                if inner.paths.contains_key(parent) {
                    break;
                }

                dirs.push(parent);
                c = parent;
            }

            dirs
        };

        let mut out = Vec::new();

        for dir in dirs.into_iter().rev() {
            let mut unit = self.allocator.unit(CreateDir(dir.to_owned()));
            let current_dependency = Dependency::Dir(unit.id);
            let dependency = *inner
                .paths
                .entry(dir.to_owned())
                .or_insert(current_dependency);

            // Someone else is creating this dependency.
            if dependency != current_dependency {
                // Other system is creating the directory, do nothing!
                if let Dependency::Dir(_) = dependency {
                    continue;
                }

                bail!(
                    "Other system is modifying path, but not as a directory: {}",
                    dir.display()
                );
            }

            unit.provides.push(dependency);

            if let Some(parent) = dir.parent() {
                unit.dependencies.extend(inner.paths.get(parent).cloned());
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

    /// Construct a relative path from a provided base directory path to the provided path
    ///
    /// ```rust
    /// use quickcfg::FileSystem;
    /// use std::path::PathBuf;
    ///
    /// let baz: PathBuf = "/foo/bar/baz".into();
    /// let bar: PathBuf = "/foo/bar".into();
    /// let quux: PathBuf = "/foo/bar/quux".into();
    /// assert_eq!(FileSystem::path_relative_from(&bar, &baz), Some("../".into()));
    /// assert_eq!(FileSystem::path_relative_from(&baz, &bar), Some("baz".into()));
    /// assert_eq!(FileSystem::path_relative_from(&quux, &baz), Some("../quux".into()));
    /// assert_eq!(FileSystem::path_relative_from(&baz, &quux), Some("../baz".into()));
    /// assert_eq!(FileSystem::path_relative_from(&bar, &quux), Some("../".into()));
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

        let mut comps = Vec::new();

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
                (Some(a), Some(Component::CurDir)) => comps.push(a),
                (Some(_), Some(Component::ParentDir)) => return None,
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

    /// Update timestamps for the given path.
    pub fn touch(path: &Path, timestamp: &SystemTime) -> Result<(), Error> {
        use filetime::FileTime;

        let accessed = FileTime::from_system_time(*timestamp);
        let modified = accessed;

        filetime::set_file_times(path, accessed, modified)
            .with_context(|| anyhow!("Failed to update timestamps for: {}", path.display()))?;
        Ok(())
    }

    /// Test if we should copy the file.
    ///
    /// This is true if:
    ///
    /// * The destination file does not exist.
    /// * The destination file has a modified timestamp less than the source file.
    fn should_copy_file(
        &self,
        from: &fs::Metadata,
        to: &Path,
        to_meta: Option<&fs::Metadata>,
        template: bool,
    ) -> Result<Option<SystemTime>, Error> {
        let from_modified = from.modified()?;

        let to_meta = match to_meta {
            Some(to_meta) => to_meta,
            None => return Ok(Some(from_modified)),
        };

        if !to_meta.is_file() {
            bail!("Exists but is not a file: {}", to.display());
        }

        let to_modified = to_meta.modified()?;

        let modified = if template {
            // use the modification time of the hierarchy if modified more recently.
            match self.data.last_modified.as_ref() {
                Some(data_modified) if from_modified < *data_modified => data_modified,
                _ => &from_modified,
            }
        } else {
            &from_modified
        };

        if *modified != to_modified {
            return Ok(Some(*modified));
        }

        Ok(None)
    }
}
