//! A unit of work. Does a single thing and DOES IT WELL.

use failure::{Error, format_err};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::{
    packages::Packages,
    hierarchy::Data,
};
use std::collections::HashSet;

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
    id: UnitId,
    /// Dependencies of this unit.
    depends: Vec<UnitId>,
    /// The unit of work.
    /// Note: box to make it cheaper to move.
    unit: Box<Unit>,
}

impl SystemUnit {
    /// Create a new system unit.
    pub fn new(id: UnitId, unit: impl Into<Unit>) -> Self {
        SystemUnit {
            id,
            depends: Vec::new(),
            unit: Box::new(unit.into()),
        }
    }

    /// Access the ID of this unit.
    pub fn id(&self) -> UnitId {
        self.id
    }

    /// Apply the unit of work.
    pub fn apply(self, input: UnitInput) -> Result<(), Error> {
        self.unit.apply(input)
    }

    /// Access dependencies of this unit.
    pub fn dependencies(&self) -> &[UnitId] {
        &self.depends
    }

    /// Register a dependency.
    pub fn dependency(&mut self, id: UnitId) {
        self.depends.push(id);
    }
}

/// The configuration to create a single directory.
#[derive(Debug)]
pub struct CreateDir(pub PathBuf);

impl CreateDir {
    fn apply(self, _: UnitInput) -> Result<(), Error> {
        use std::fs;

        let CreateDir(dir) = self;
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
        use log::info;
        use handlebars::Handlebars;
        use std::fs::{self, File};
        use std::io::Write;

        let CopyFile(from, to) = self;

        let UnitInput { data, .. } = input;

        // NB: render template.
        let content = fs::read_to_string(&from)?;
        let handlebars = Handlebars::new();
        let out = handlebars.render_template(&content, &data.0)?;

        info!("{} -> {}", from.display(), to.display());
        File::create(&to)?.write_all(out.as_bytes())?;
        Ok(())
    }
}

/// Install a number of packages.
#[derive(Debug)]
pub struct InstallPackages(pub HashSet<String>);

impl InstallPackages {
    fn apply(self, input: UnitInput) -> Result<(), Error> {
        use log::info;

        let UnitInput {
            packages,
            ..
        } = input;

        let _packages = packages.ok_or_else(|| format_err!("no package manager available to install packages"))?;

        let InstallPackages(packages) = self;

        info!("WOULD INSTALL: {:?}", packages);
        Ok(())
    }
}

impl From<InstallPackages> for Unit {
    fn from(value: InstallPackages) -> Unit {
        Unit::InstallPackages(value)
    }
}
