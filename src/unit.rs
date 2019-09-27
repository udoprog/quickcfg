//! A unit of work. Does a single thing and DOES IT WELL.

use crate::{
    git::GitSystem, hierarchy::Data, os, packages, packages::PackageManager, state::State,
    FileSystem,
};
use failure::{format_err, Error, Fail, ResultExt};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dependency {
    /// Someone providing a file.
    /// The file is aliased by FileSystem.
    File(UnitId),
    /// Someone providing a directory.
    /// The file is aliased by FileSystem.
    Dir(UnitId),
    /// Direct dependency on other unit.
    Unit(UnitId),
}

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
pub struct UnitInput<'a, 's> {
    /// Primary package manager.
    pub packages: &'a packages::Provider,
    /// Data loaded from the hierarchy.
    pub data: &'a Data,
    /// Read-only state.
    /// If none, the read state is the mutated state.
    pub read_state: &'a State<'s>,
    /// Unit-local state that can be mutated.
    pub state: &'a mut State<'s>,
    /// Current timestamp.
    pub now: &'a SystemTime,
    /// Current git system.
    pub git_system: &'a dyn GitSystem,
}

/// Declare unit enum.
macro_rules! unit {
    ($($name:ident,)*) => {
        /// A single unit of work.
        #[derive(Debug)]
        pub enum Unit {
            System,
            $($name($name),)*
        }

        impl Unit {
            pub fn apply(&self, input: UnitInput) -> Result<(), Error> {
                use self::Unit::*;

                let res = match *self {
                    // do nothing.
                    System => Ok(()),
                    // do something.
                    $($name(ref unit) => unit.apply(input),)*
                };

                Ok(res.with_context(|_| format_err!("Failed to run unit: {:?}", self))?)
            }
        }

        impl fmt::Display for Unit {
            fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                use self::Unit::*;

                match *self {
                    System => write!(fmt, "system unit"),
                    $($name(ref unit) => unit.fmt(fmt),)*
                }
            }
        }
    }
}

unit![
    CopyFile,
    CopyTemplate,
    Symlink,
    CreateDir,
    Install,
    Download,
    AddMode,
    RunOnce,
    GitClone,
    GitUpdate,
];

/// A system unit, which is a unit coupled with a set of dependencies.
#[derive(Debug)]
pub struct SystemUnit {
    /// The ID of this unit.
    pub id: UnitId,
    /// Dependencies of this unit.
    pub dependencies: Vec<Dependency>,
    /// Dependencies that this unit provides.
    pub provides: Vec<Dependency>,
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
            provides: Vec::new(),
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
#[derive(Debug, Hash)]
pub struct CopyFile {
    /// The source file.
    pub from: PathBuf,
    /// Source file modification time.
    pub from_modified: SystemTime,
    /// The destination file.
    pub to: PathBuf,
}

impl fmt::Display for CopyFile {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "copy file {} -> {}",
            self.from.display(),
            self.to.display()
        )
    }
}

impl CopyFile {
    fn apply(&self, _: UnitInput) -> Result<(), Error> {
        use std::fs::File;
        use std::io;

        let CopyFile {
            ref from,
            ref from_modified,
            ref to,
        } = *self;

        log::info!("{} -> {}", from.display(), to.display());
        io::copy(&mut File::open(from)?, &mut File::create(to)?)?;
        // make sure timestamp is in sync.
        FileSystem::touch(&to, from_modified)
    }
}

impl From<CopyFile> for Unit {
    fn from(value: CopyFile) -> Unit {
        Unit::CopyFile(value)
    }
}

/// The configuration for a unit to copy a single file.
#[derive(Debug, Hash)]
pub struct CopyTemplate {
    /// The source file.
    pub from: PathBuf,
    /// Source file modification time.
    pub from_modified: SystemTime,
    /// The destination file.
    pub to: PathBuf,
    /// If the destination file exists, we assume that its content is the same.
    pub to_exists: bool,
}

impl fmt::Display for CopyTemplate {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "template file {} -> {}",
            self.from.display(),
            self.to.display()
        )
    }
}

impl CopyTemplate {
    /// Construct the ID for this unit.
    fn id(&self) -> String {
        use std::hash::{Hash, Hasher};

        let mut state = fxhash::FxHasher64::default();
        self.hash(&mut state);

        format!("copy-template/{:x}", state.finish())
    }

    fn apply(&self, input: UnitInput) -> Result<(), Error> {
        use handlebars::{Context, Handlebars, Output, RenderContext, Renderable, Template};
        use std::fs::{self, File};
        use std::io::{self, Cursor, Write};

        let CopyTemplate {
            ref from,
            ref from_modified,
            ref to,
            to_exists,
        } = *self;

        let UnitInput {
            data,
            read_state,
            state,
            ..
        } = input;

        // We do some extra work in here that we would usually split up into more units.
        // The reason is that we can't efficiently determine if that work should be done without
        // doing a lot of it up front.
        //
        // This includes:
        // * Reading the template file to determine which database variables to use.

        let content = fs::read_to_string(&from)
            .map_err(|e| format_err!("failed to read path: {}: {}", from.display(), e))?;

        let data = data.load_from_spec(&content).map_err(|e| {
            format_err!(
                "failed to load hierarchy for path: {}: {}",
                from.display(),
                e
            )
        })?;

        let id = self.id();
        let hash = (&data, &content);

        if to_exists && read_state.is_hash_fresh(&id, &hash)? {
            // Nothing about the template would change, only update the modified time of the file.
            log::info!("touching {}", to.display());
            // only need to update timestamp.
            return FileSystem::touch(&to, from_modified);
        }

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

        log::info!("{} -> {} (template)", from.display(), to.display());
        File::create(&to)?.write_all(&out)?;
        state.touch_hash(&id, &hash)?;
        return FileSystem::touch(&to, from_modified);

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

impl From<CopyTemplate> for Unit {
    fn from(value: CopyTemplate) -> Unit {
        Unit::CopyTemplate(value)
    }
}

/// The configuration for a unit to create a symlink.
#[derive(Debug)]
pub struct Symlink {
    /// `true` if the destination file needs to be removed.
    pub remove: bool,
    /// destination file to create.
    pub path: PathBuf,
    /// link to set up.
    pub link: PathBuf,
}

impl fmt::Display for Symlink {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "link file {} to {}",
            self.path.display(),
            self.link.display()
        )
    }
}

impl Symlink {
    fn apply(&self, _: UnitInput) -> Result<(), Error> {
        os::create_symlink(self)
    }
}

impl From<Symlink> for Unit {
    fn from(value: Symlink) -> Unit {
        Unit::Symlink(value)
    }
}

/// Install a number of packages.
#[derive(Debug)]
pub struct Install {
    pub package_manager: Arc<dyn PackageManager>,
    pub all_packages: BTreeSet<String>,
    pub to_install: Vec<String>,
    pub id: String,
}

impl fmt::Display for Install {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        if self.to_install.is_empty() {
            return write!(fmt, "install packages");
        }

        let names = self.to_install.join(", ");
        write!(fmt, "{}: install packages: {}", self.id, names)
    }
}

impl Install {
    fn apply(&self, input: UnitInput) -> Result<(), Error> {
        let UnitInput { state, .. } = input;

        let Install {
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

impl From<Install> for Unit {
    fn from(value: Install) -> Unit {
        Unit::Install(value)
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

/// Mode modifications to apply.
#[repr(u32)]
pub enum Mode {
    Execute = 1,
    Read = 2,
    Write = 4,
}

/// Change the permissions of the given file.
#[derive(Debug)]
pub struct AddMode {
    pub path: PathBuf,
    user: u32,
    group: u32,
    other: u32,
}

impl AddMode {
    /// Create a new add mode unit.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_owned(),
            user: 0,
            group: 0,
            other: 0,
        }
    }

    /// If the added mode is executable.
    pub fn is_executable(&self) -> bool {
        if self.user & (Mode::Execute as u32) != 0 {
            return true;
        }

        if self.group & (Mode::Execute as u32) != 0 {
            return true;
        }

        if self.other & (Mode::Execute as u32) != 0 {
            return true;
        }

        false
    }

    /// Modify user mode.
    pub fn user(mut self, mode: Mode) -> Self {
        self.user |= mode as u32;
        self
    }

    /// Modify group mode.
    pub fn group(mut self, mode: Mode) -> Self {
        self.group |= mode as u32;
        self
    }

    /// Modify other mode.
    pub fn other(mut self, mode: Mode) -> Self {
        self.other |= mode as u32;
        self
    }
}

impl AddMode {
    /// Convert into a unix mode.
    pub fn unix_mode(&self) -> u32 {
        let AddMode {
            user, group, other, ..
        } = *self;

        (user << (3 * 2)) + (group << 3) + other
    }
}

impl fmt::Display for AddMode {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "add mode {:o} to {}",
            self.unix_mode(),
            self.path.display()
        )
    }
}

impl AddMode {
    fn apply(&self, _: UnitInput) -> Result<(), Error> {
        os::add_mode(self)
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
    /// Arguments to add when running the command.
    pub args: Vec<String>,
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
            args: Vec::new(),
        }
    }

    /// Apply the unit.
    fn apply(&self, input: UnitInput) -> Result<(), Error> {
        use crate::command::Command;
        use std::borrow::Cow;
        use std::ffi::OsStr;

        let UnitInput { state, .. } = input;

        let RunOnce {
            ref id,
            ref path,
            shell,
            ref args,
        } = *self;

        log::info!("running {}", path.display());

        let mut command_args = Vec::new();

        let cmd = if shell {
            command_args.push(path.as_os_str());
            Command::new(Cow::from(Path::new(Self::BIN_SH)))
        } else {
            Command::new(Cow::from(path))
        };

        for arg in args {
            command_args.push(OsStr::new(arg.as_str()));
        }

        let output = cmd
            .run(&command_args)
            .with_context(|_| format_err!("Failed to run: {}", path.display()))?;

        if !output.status.success() {
            return Err(Error::from(output.into_error()));
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

/// Run the given executable once.
#[derive(Debug)]
pub struct GitClone {
    /// The ID of the thing being cloned.
    pub id: String,
    /// Remote to clone.
    pub remote: String,
    /// Git repository.
    pub path: PathBuf,
}

impl fmt::Display for GitClone {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "git clone `{}` to `{}`",
            self.remote,
            self.path.display()
        )
    }
}

impl GitClone {
    /// Apply the unit.
    fn apply(&self, input: UnitInput) -> Result<(), Error> {
        let UnitInput {
            state, git_system, ..
        } = input;

        let GitClone {
            ref id,
            ref remote,
            ref path,
        } = *self;

        log::info!("Cloning `{}` into `{}`", remote, path.display());
        GitSystem::clone(git_system, remote, path)?;
        state.touch(&id);
        Ok(())
    }
}

impl From<GitClone> for Unit {
    fn from(value: GitClone) -> Unit {
        Unit::GitClone(value)
    }
}

/// Run the given executable once.
#[derive(Debug)]
pub struct GitUpdate {
    /// The ID of the thing being cloned.
    pub id: String,
    /// Git repository.
    pub path: PathBuf,
    /// If the update should be forced.
    pub force: bool,
}

impl fmt::Display for GitUpdate {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "git update: {}", self.path.display())
    }
}

impl GitUpdate {
    /// Apply the unit.
    fn apply(&self, input: UnitInput) -> Result<(), Error> {
        let UnitInput {
            state, git_system, ..
        } = input;

        let GitUpdate {
            ref id,
            ref path,
            force,
        } = *self;

        let git = git_system.open(path)?;

        if git.needs_update()? {
            if force {
                log::info!("Force updating `{}`", git.path().display());
                git.force_update()?;
            } else {
                log::info!("Updating `{}`", git.path().display());
                git.update()?;
            }
        }

        state.touch(&id);
        Ok(())
    }
}

impl From<GitUpdate> for Unit {
    fn from(value: GitUpdate) -> Unit {
        Unit::GitUpdate(value)
    }
}
