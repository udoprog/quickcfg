//! Things to do.

use crate::{
    environment as e, git, packages, state::State, Data, Facts, FileSystem, Opts, SystemUnit,
    Timestamp, UnitAllocator, UnitId,
};
use anyhow::Error;
use directories::BaseDirs;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

#[macro_use]
mod macros;
mod copy_dir;
mod download;
mod download_and_run;
mod from_db;
mod git_sync;
mod install;
mod link;
mod link_dir;
mod only_for;

use self::copy_dir::CopyDir;
use self::download::Download;
use self::download_and_run::DownloadAndRun;
use self::from_db::FromDb;
use self::git_sync::GitSync;
use self::install::Install;
use self::link::Link;
use self::link_dir::LinkDir;
use self::only_for::OnlyFor;

/// What should happen after a system has been translated.
pub enum Translation<'a> {
    /// Keep the current system.
    Keep,
    /// Discard the current system.
    Discard,
    /// Expand and discard the current system into the given collection of systems.
    Expand(&'a [System]),
}

macro_rules! system_impl {
    ($($name:ident,)*) => {
        impl System {
            pub fn translate(&self) -> Translation<'_> {
                use self::System::*;

                match self {
                    $($name(system) => system.translate(),)*
                }
            }

            /// Get the id of this system.
            pub fn id(&self) -> Option<&str> {
                use self::System::*;

                match self {
                    $($name(system) => system.id(),)*
                }
            }

            /// Get all things that this system depends on.
            pub fn requires(&self) -> &[String] {
                use self::System::*;

                match self {
                    $($name(system) => system.requires(),)*
                }
            }

            /// Apply changes for this system.
            #[allow(unused)]
            pub fn apply<E>(&self, input: $crate::system::SystemInput<E>)
                -> Result<Vec<$crate::system::SystemUnit>, Error>
            where
                E: Copy + $crate::environment::Environment,
            {
                use anyhow::{Context as _, anyhow};
                use self::System::*;

                let res = match self {
                    $($name(system) => system.apply(input),)*
                };

                Ok(res.with_context(|| anyhow!("Failed to run system: {:?}", self))?)
            }
        }

        impl fmt::Display for System {
            fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                match *self {
                    $(
                    System::$name(ref system) => {
                        if let Some(id) = system.id() {
                            write!(fmt, "{}: {}", id, system)
                        } else {
                            system.fmt(fmt)
                        }
                    }
                    )*
                }
            }
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum System {
    #[serde(rename = "copy-dir")]
    CopyDir(CopyDir),
    #[serde(rename = "link-dir")]
    LinkDir(LinkDir),
    #[serde(rename = "install")]
    Install(Install),
    #[serde(rename = "download-and-run")]
    DownloadAndRun(DownloadAndRun),
    #[serde(rename = "download")]
    Download(Download),
    #[serde(rename = "link")]
    Link(Link),
    #[serde(rename = "git-sync")]
    GitSync(GitSync),
    #[serde(rename = "only-for")]
    OnlyFor(OnlyFor),
    #[serde(rename = "from-db")]
    FromDb(FromDb),
}

system_impl![
    CopyDir,
    LinkDir,
    Install,
    DownloadAndRun,
    Download,
    Link,
    GitSync,
    OnlyFor,
    FromDb,
];

/// All inputs for a system.
#[derive(Clone, Copy)]
pub struct SystemInput<'a, 'f, E>
where
    E: e::Environment,
{
    /// The root directory of the project being built.
    pub root: &'a Path,
    /// Known base directories to use.
    pub base_dirs: Option<&'a BaseDirs>,
    /// Set of facts.
    pub facts: &'a Facts,
    /// Data loaded from hierarchy.
    pub data: &'a Data,
    /// Source of environment variables.
    pub environment: E,
    /// Detected primary package manager for the system.
    pub packages: &'a packages::Provider,
    /// Unit allocator to use.
    pub allocator: &'a UnitAllocator,
    /// File utilities.
    pub file_system: &'a FileSystem<'f>,
    /// State accessor.
    pub state: &'a State<'a>,
    /// Current time.
    pub now: Timestamp,
    /// Current optsion.
    pub opts: &'a Opts,
    /// The current git system.
    pub git_system: &'a dyn git::GitSystem,
}

/// Helper structure used to resolve dependencies.
pub enum Dependency<'a> {
    /// Transitive dependency, where we have to look up other systems to fully resolve.
    Transitive(&'a [String]),
    /// Direct dependency to another unit.
    Direct(UnitId),
    /// No dependencies.
    None,
}

impl Default for Dependency<'_> {
    fn default() -> Self {
        Dependency::None
    }
}

impl<'a> Dependency<'a> {
    /// Resolve all unit dependencies for the current dependency.
    pub fn resolve(
        &self,
        systems: &HashMap<&'a str, Dependency<'a>>,
    ) -> impl IntoIterator<Item = crate::unit::Dependency> {
        use std::collections::VecDeque;

        let mut ids = Vec::new();

        let mut queue = VecDeque::new();
        queue.push_back(self);

        while let Some(dependency) = queue.pop_front() {
            match *dependency {
                Dependency::Transitive(requires) => {
                    for id in requires {
                        queue.extend(systems.get(id.as_str()));
                    }
                }
                Dependency::Direct(id) => ids.push(crate::unit::Dependency::Unit(id)),
                Dependency::None => continue,
            }
        }

        ids
    }
}
