//! Thread-safe utilities for creating files and directories.
//! use std::collections::HashMap;
//!
use crate::{
    hierarchy::Data,
    opts::Opts,
    system::System,
    unit::{
        CopyFile, CopyTemplate, CreateDir, Dependency, Symlink, SystemUnit, UnitAllocator, UnitId,
    },
};
use failure::{bail, format_err, Error, ResultExt};
use fxhash::FxHashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Utilities to build units for creating directories.
pub struct GlobalFileUtils<'a> {
    opts: &'a Opts,
    state_dir: PathBuf,
    allocator: &'a UnitAllocator,
    data: &'a Data,
    pub valid: bool,
    pub directories: FxHashMap<PathBuf, Vec<&'a System>>,
    pub files: FxHashMap<PathBuf, Vec<&'a System>>,
}

impl<'a> GlobalFileUtils<'a> {
    /// Create new, thread-safe file utilities.
    pub fn new(
        opts: &'a Opts,
        state_dir: &Path,
        allocator: &'a UnitAllocator,
        data: &'a Data,
    ) -> GlobalFileUtils<'a> {
        GlobalFileUtils {
            opts,
            state_dir: state_dir.to_owned(),
            allocator,
            data,
            valid: true,
            directories: FxHashMap::default(),
            files: FxHashMap::default(),
        }
    }

    /// Create a new child utility.
    pub fn new_child(&self) -> FileUtils<'a> {
        FileUtils {
            opts: self.opts,
            state_dir: self.state_dir.to_owned(),
            allocator: self.allocator,
            data: self.data,
            directories: FxHashMap::default(),
            files: FxHashMap::default(),
        }
    }

    /// Extend the state of this file utils with another.
    /// This sets `valid` to false if there are multiple systems modifying the same directory or
    /// file, and keeps track of which systems are trying to do that.
    pub fn extend(&mut self, system: &'a System, other: FileUtils) -> bool {
        for (key, _) in other.directories {
            let systems = self.directories.entry(key.clone()).or_default();
            self.valid = self.valid && systems.is_empty();
            systems.push(system);
        }

        for (key, _) in other.files {
            let systems = self.files.entry(key.clone()).or_default();
            self.valid = self.valid && systems.is_empty();
            systems.push(system);
        }

        self.valid
    }
}

/// Utilities to build units for creating directories.
pub struct FileUtils<'a> {
    opts: &'a Opts,
    state_dir: PathBuf,
    allocator: &'a UnitAllocator,
    data: &'a Data,
    directories: FxHashMap<PathBuf, UnitId>,
    files: FxHashMap<PathBuf, UnitId>,
}

impl<'a> FileUtils<'a> {
    /// Access or allocate a file dependency of the given path.
    pub fn file_dependency(&mut self, path: &Path) -> Dependency {
        if let Some(id) = self.files.get(path).cloned() {
            return Dependency::File(id);
        }

        let id = self.allocator.allocate();
        self.files.insert(path.to_owned(), id);
        Dependency::File(id)
    }

    /// Access or allocate a directory dependency of the given path.
    pub fn dir_dependency(&mut self, path: &Path) -> Dependency {
        if let Some(id) = self.directories.get(path).cloned() {
            return Dependency::Dir(id);
        }

        let id = self.allocator.allocate();
        self.directories.insert(path.to_owned(), id);
        Dependency::Dir(id)
    }

    /// Try to create a symlink.
    pub fn symlink(
        &mut self,
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
                unit.dependencies.push(self.dir_dependency(parent));
            }
        }

        unit.provides.push(self.file_dependency(path));
        Ok(Some(unit))
    }

    /// Optionally set up if we should copy a file.
    ///
    /// This is true if:
    ///
    /// * The destination file does not exist.
    /// * The destination file has a modified timestamp less than the source file.
    pub fn copy_file(
        &mut self,
        from: &Path,
        from_meta: fs::Metadata,
        to: &Path,
        to_meta: Option<&fs::Metadata>,
        template: bool,
    ) -> Result<Option<SystemUnit>, Error> {
        let from_modified = match self.should_copy_file(&from_meta, &to, to_meta, template)? {
            Some(modified) => modified,
            None => return Ok(None),
        };

        let mut unit = match template {
            true => self.allocator.unit(CopyTemplate {
                from: from.to_owned(),
                from_modified,
                to: to.to_owned(),
                to_exists: to_meta.is_some(),
            }),
            false => self.allocator.unit(CopyFile {
                from: from.to_owned(),
                from_modified,
                to: to.to_owned(),
            }),
        };

        if let Some(parent) = to.parent() {
            if !parent.is_dir() {
                unit.dependencies.push(self.dir_dependency(parent));
            }
        }

        unit.provides.push(self.file_dependency(to));
        Ok(Some(unit))
    }

    /// Recursively set up units with dependencies to create the given directories.
    pub fn create_dir_all(&mut self, dir: &Path) -> Result<Vec<SystemUnit>, Error> {
        let dirs = {
            // Directory is already being created.
            if self.directories.contains_key(dir) {
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

                if self.directories.contains_key(parent) {
                    break;
                }

                dirs.push(parent);
                c = parent;
            }

            dirs
        };

        let mut out = Vec::new();

        for dir in dirs.into_iter().rev() {
            // needs to re-check now that we have mutable access.
            if self.directories.contains_key(dir) {
                continue;
            }

            let mut unit = self.allocator.unit(CreateDir(dir.to_owned()));
            self.directories.insert(dir.to_owned(), unit.id);
            unit.provides.push(Dependency::Dir(unit.id));

            if let Some(parent) = dir.parent() {
                unit.dependencies
                    .extend(self.directories.get(parent).cloned().map(Dependency::Dir));
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

    /// Update timestamps for the given path.
    pub fn touch(path: &Path, timestamp: &SystemTime) -> Result<(), Error> {
        use filetime::{self, FileTime};

        let accessed = FileTime::from_system_time(timestamp.clone());
        let modified = accessed.clone();

        filetime::set_file_times(path, accessed, modified)
            .with_context(|_| format_err!("Failed to update timestamps for: {}", path.display()))?;
        return Ok(());
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

        if template {
            if let Some(modified) = self.data.last_modified.as_ref() {
                if *modified != to_modified {
                    return Ok(Some(modified.clone()));
                }
            } else {
                if from_modified != to_modified {
                    return Ok(Some(from_modified));
                }
            }
        } else {
            if from_modified != to_modified {
                return Ok(Some(from_modified));
            }
        }

        Ok(None)
    }
}
