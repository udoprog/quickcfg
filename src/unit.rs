//! A unit of work. Does a single thing and DOES IT WELL.

use crate::{hierarchy::Data, packages, packages::PackageManager, state::State};
use failure::{bail, format_err, Error, Fail, ResultExt};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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
pub struct UnitInput<'a, 's, 'c: 's> {
    /// Primary package manager.
    pub packages: &'a packages::Provider,
    /// Data loaded from the hierarchy.
    pub data: &'a Data,
    /// Unit-local state.
    pub state: &'s mut State<'c>,
}

/// A single unit of work.
#[derive(Debug)]
pub enum Unit {
    System,
    CopyFile(CopyFile),
    CreateDir(CreateDir),
    InstallPackages(InstallPackages),
    Download(Download),
    AddMode(AddMode),
    RunOnce(RunOnce),
}

impl From<CopyFile> for Unit {
    fn from(value: CopyFile) -> Unit {
        Unit::CopyFile(value)
    }
}

impl Unit {
    pub fn apply(&self, input: UnitInput) -> Result<(), Error> {
        use self::Unit::*;

        let res = match *self {
            // do nothing.
            System => Ok(()),
            // do something.
            CopyFile(ref unit) => unit.apply(input),
            CreateDir(ref unit) => unit.apply(input),
            InstallPackages(ref unit) => unit.apply(input),
            Download(ref unit) => unit.apply(input),
            AddMode(ref unit) => unit.apply(input),
            RunOnce(ref unit) => unit.apply(input),
        };

        Ok(res.with_context(|_| format_err!("Failed to run unit: {:?}", self))?)
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::Unit::*;

        match *self {
            System => write!(fmt, "system unit"),
            CopyFile(ref unit) => unit.fmt(fmt),
            CreateDir(ref unit) => unit.fmt(fmt),
            InstallPackages(ref unit) => unit.fmt(fmt),
            Download(ref unit) => unit.fmt(fmt),
            AddMode(ref unit) => unit.fmt(fmt),
            RunOnce(ref unit) => unit.fmt(fmt),
        }
    }
}

/// A system unit, which is a unit coupled with a set of dependencies.
#[derive(Debug)]
pub struct SystemUnit {
    /// The ID of this unit.
    pub id: UnitId,
    /// Dependencies of this unit.
    pub dependencies: Vec<UnitId>,
    /// Whether the unit needs access to the main thread. For example, for user input.
    pub thread_local: bool,
    /// The unit of work.
    /// Note: box to make it cheaper to move.
    unit: Box<Unit>,
}

impl fmt::Display for SystemUnit {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "unit({:03}): {} (depends: {:?})",
            self.id, self.unit, self.dependencies
        )
    }
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
    pub fn apply(&self, input: UnitInput) -> Result<(), Error> {
        self.unit.apply(input)
    }
}

/// The configuration to create a single directory.
#[derive(Debug)]
pub struct CreateDir(pub PathBuf);

impl fmt::Display for CreateDir {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "create directory {}", self.0.display())
    }
}

impl CreateDir {
    fn apply(&self, _: UnitInput) -> Result<(), Error> {
        use std::fs;
        let CreateDir(ref dir) = self;
        log::info!("creating dir: {}", dir.display());
        fs::create_dir(dir)?;
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
pub struct CopyFile {
    pub from: PathBuf,
    pub to: PathBuf,
    pub templates: bool,
}

impl fmt::Display for CopyFile {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "copy file {} -> {} (template: {})",
            self.from.display(),
            self.to.display(),
            self.templates
        )
    }
}

impl CopyFile {
    fn apply(&self, input: UnitInput) -> Result<(), Error> {
        use std::fs::{self, File};
        use std::io::{self, Write};

        let CopyFile {
            ref from,
            ref to,
            templates,
        } = *self;

        let UnitInput { data, .. } = input;

        if templates {
            log::info!("{} -> {} (template)", from.display(), to.display());
            let out = render(&from, data).with_context(|_| RenderError(from.to_owned()))?;
            File::create(&to)?.write_all(out.as_bytes())?;
        } else {
            log::info!("{} -> {}", from.display(), to.display());
            io::copy(&mut File::open(from)?, &mut File::create(to)?)?;
        }

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
pub struct InstallPackages {
    pub package_manager: Arc<dyn PackageManager>,
    pub all_packages: BTreeSet<String>,
    pub to_install: Vec<String>,
    pub id: String,
}

impl fmt::Display for InstallPackages {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.to_install.is_empty() {
            return write!(fmt, "install packages");
        }

        let names = self.to_install.join(", ");
        write!(fmt, "{}: install packages: {}", self.id, names)
    }
}

impl InstallPackages {
    fn apply(&self, input: UnitInput) -> Result<(), Error> {
        let UnitInput { state, .. } = input;

        let InstallPackages {
            ref package_manager,
            ref all_packages,
            ref to_install,
            ref id,
        } = *self;

        if !to_install.is_empty() {
            let names = to_install.join(", ");
            log::info!("Installing packages for `{}`: {}", id, names);
            package_manager.install_packages(to_install)?;
        }

        state.touch_hash(id, &all_packages)?;
        Ok(())
    }
}

impl From<InstallPackages> for Unit {
    fn from(value: InstallPackages) -> Unit {
        Unit::InstallPackages(value)
    }
}

/// Download the given URL as an executable and write to the given path.
#[derive(Debug)]
pub struct Download(pub reqwest::Url, pub PathBuf);

impl fmt::Display for Download {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "download {} to {}", self.0, self.1.display())
    }
}

impl Download {
    fn apply(&self, input: UnitInput) -> Result<(), Error> {
        use std::fs::File;
        let UnitInput { .. } = input;
        let Download(ref url, ref path) = *self;

        let mut out = File::create(&path)
            .with_context(|_| format_err!("Failed to open file: {}", path.display()))?;

        let mut response = reqwest::get(url.clone())
            .with_context(|_| format_err!("Failed to download URL: {}", url))?;

        response.copy_to(&mut out)?;
        Ok(())
    }
}

impl From<Download> for Unit {
    fn from(value: Download) -> Unit {
        Unit::Download(value)
    }
}

/// Change the permissions of the given file.
#[derive(Debug)]
pub struct AddMode(pub PathBuf, pub u32);

impl fmt::Display for AddMode {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "add mode {} to {}", self.1, self.0.display())
    }
}

impl AddMode {
    fn apply(&self, input: UnitInput) -> Result<(), Error> {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let UnitInput { .. } = input;
        let AddMode(ref path, mode) = *self;

        let mut perm = path.metadata()?.permissions();
        let mode = perm.mode() | mode;
        perm.set_mode(mode);

        fs::set_permissions(&path, perm)
            .with_context(|_| format_err!("failed to add mode: {}", path.display()))?;

        Ok(())
    }
}

impl From<AddMode> for Unit {
    fn from(value: AddMode) -> Unit {
        Unit::AddMode(value)
    }
}

/// Run the given executable once.
#[derive(Debug)]
pub struct RunOnce {
    /// ID to mark once run.
    pub id: String,
    /// Path to run.
    pub path: PathBuf,
    /// Run using a shell.
    pub shell: bool,
}

impl fmt::Display for RunOnce {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "run `{}` once as `{}`", self.path.display(), self.id)
    }
}

impl RunOnce {
    const BIN_SH: &'static str = "/bin/sh";

    /// Construct a new RunOnce.
    pub fn new(id: String, path: PathBuf) -> RunOnce {
        RunOnce {
            id,
            path,
            shell: false,
        }
    }

    /// Apply the unit.
    fn apply(&self, input: UnitInput) -> Result<(), Error> {
        use std::process::Command;
        let UnitInput { state, .. } = input;

        let RunOnce {
            ref id,
            ref path,
            shell,
        } = *self;

        log::info!("Running {}", path.display());

        let mut cmd = if shell {
            let mut cmd = Command::new(Self::BIN_SH);
            cmd.arg(&path);
            cmd
        } else {
            Command::new(&path)
        };

        let status = cmd
            .status()
            .with_context(|_| format_err!("Failed to run: {}", path.display()))?;

        if !status.success() {
            bail!(
                "Command `{}` exited with non-zero status: {:?}",
                path.display(),
                status
            );
        }

        state.touch_once(&id);
        Ok(())
    }
}

impl From<RunOnce> for Unit {
    fn from(value: RunOnce) -> Unit {
        Unit::RunOnce(value)
    }
}
