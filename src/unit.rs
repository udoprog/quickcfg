//! A unit of work. Does a single thing and DOES IT WELL.

use crate::{hierarchy::Data, packages::Packages};
use failure::{format_err, Error, Fail, ResultExt};
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Fail, Debug)]
pub struct RenderError(PathBuf);

impl fmt::Display for RenderError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "failed to render: {}", self.0.display())
    }
}

pub type UnitId = usize;

#[derive(Debug, Default)]
pub struct UnitAllocator {
    current: AtomicUsize,
}

impl UnitAllocator {
    /// Create a new system unit.
    pub fn unit(&self, unit: impl Into<Unit>) -> SystemUnit {
        let id = self.allocate();
        SystemUnit::new(id, unit)
    }

    /// Allocate a new unit id.
    pub fn allocate(&self) -> UnitId {
        self.current.fetch_add(1, Ordering::Relaxed)
    }
}

/// All inputs for a system.
#[derive(Clone, Copy)]
pub struct UnitInput<'a> {
    /// Primary package manager.
    pub packages: Option<&'a Packages>,
    /// Data loaded from the hierarchy.
    pub data: &'a Data,
}

/// A single unit of work.
#[derive(Debug)]
pub enum Unit {
    System,
    CopyFile(CopyFile),
    CreateDir(CreateDir),
    InstallPackages(InstallPackages),
}

impl From<CopyFile> for Unit {
    fn from(value: CopyFile) -> Unit {
        Unit::CopyFile(value)
    }
}

impl Unit {
    pub fn apply(self, input: UnitInput) -> Result<(), Error> {
        use self::Unit::*;

        match self {
            // do nothing.
            System => Ok(()),
            CopyFile(unit) => unit.apply(input),
            CreateDir(unit) => unit.apply(input),
            InstallPackages(unit) => unit.apply(input),
        }
    }
}

/// A system unit, which is a unit coupled with a set of dependencies.
#[derive(Debug)]
pub struct SystemUnit {
    /// The ID of this unit.
    pub id: UnitId,
    /// Dependencies of this unit.
    dependencies: Vec<UnitId>,
    /// Whether the unit needs access to the main thread. For example, for user input.
    pub thread_local: bool,
    /// The unit of work.
    /// Note: box to make it cheaper to move.
    unit: Box<Unit>,
}

impl SystemUnit {
    /// Create a new system unit.
    pub fn new(id: UnitId, unit: impl Into<Unit>) -> Self {
        SystemUnit {
            id,
            dependencies: Vec::new(),
            thread_local: false,
            unit: Box::new(unit.into()),
        }
    }

    /// Apply the unit of work.
    pub fn apply(self, input: UnitInput) -> Result<(), Error> {
        self.unit.apply(input)
    }

    /// Access dependencies of this unit.
    pub fn dependencies(&self) -> &[UnitId] {
        &self.dependencies
    }

    /// Register a dependency.
    pub fn add_dependency(&mut self, id: UnitId) {
        self.dependencies.push(id);
    }

    /// Add a set of dependencies..
    pub fn add_dependencies(&mut self, ids: impl IntoIterator<Item = UnitId>) {
        self.dependencies.extend(ids);
    }
}

/// The configuration to create a single directory.
#[derive(Debug)]
pub struct CreateDir(pub PathBuf);

impl CreateDir {
    fn apply(self, _: UnitInput) -> Result<(), Error> {
        use std::fs;
        let CreateDir(dir) = self;
        log::info!("creating dir: {}", dir.display());
        fs::create_dir(&dir)?;
        Ok(())
    }
}

impl From<CreateDir> for Unit {
    fn from(value: CreateDir) -> Unit {
        Unit::CreateDir(value)
    }
}

/// The configuration for a unit to copy a single file.
#[derive(Debug)]
pub struct CopyFile(pub PathBuf, pub PathBuf);

impl CopyFile {
    fn apply(self, input: UnitInput) -> Result<(), Error> {
        use std::fs::{self, File};
        use std::io::Write;

        let CopyFile(from, to) = self;

        let UnitInput { data, .. } = input;

        let out = render(&from, data).with_context(|_| RenderError(from.to_owned()))?;

        log::info!("{} -> {}", from.display(), to.display());
        File::create(&to)?.write_all(out.as_bytes())?;
        return Ok(());

        fn render(from: &Path, data: &Data) -> Result<String, Error> {
            use handlebars::{Context, Handlebars, Output, RenderContext, Renderable, Template};
            use std::io::{self, Cursor, Write};

            let content = fs::read_to_string(&from)
                .map_err(|e| format_err!("failed to read path: {}: {}", from.display(), e))?;

            let data = data.load_from_spec(&content).map_err(|e| {
                format_err!(
                    "failed to load hierarchy for path: {}: {}",
                    from.display(),
                    e
                )
            })?;

            let reg = Handlebars::new();

            let mut out = Vec::<u8>::new();

            let mut tpl = Template::compile2(&content, true)?;
            tpl.name = Some(from.display().to_string());

            tpl.render(
                &reg,
                &Context::wraps(&data)?,
                &mut RenderContext::new(None),
                &mut WriteOutput::new(Cursor::new(&mut out)),
            )?;

            return Ok(String::from_utf8(out)?);

            pub struct WriteOutput<W: Write> {
                write: W,
            }

            impl<W: Write> Output for WriteOutput<W> {
                fn write(&mut self, seg: &str) -> Result<(), io::Error> {
                    self.write.write_all(seg.as_bytes())
                }
            }

            impl<W: Write> WriteOutput<W> {
                pub fn new(write: W) -> WriteOutput<W> {
                    WriteOutput { write }
                }
            }
        }
    }
}

/// Install a number of packages.
#[derive(Debug)]
pub struct InstallPackages(pub HashSet<String>);

impl InstallPackages {
    fn apply(self, input: UnitInput) -> Result<(), Error> {
        let UnitInput { packages, .. } = input;

        let packages = packages
            .ok_or_else(|| format_err!("no package manager available to install packages"))?;

        let InstallPackages(packages_to_install) = self;

        let names = packages_to_install
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        log::info!("Installing missing packages: {}", names);
        packages.install_packages(&packages_to_install)
    }
}

impl From<InstallPackages> for Unit {
    fn from(value: InstallPackages) -> Unit {
        Unit::InstallPackages(value)
    }
}
