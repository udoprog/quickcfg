//! Things to do.

use crate::{
    environment as e,
    facts::Facts,
    git,
    hierarchy::Data,
    opts::Opts,
    packages,
    state::State,
    unit::{self, SystemUnit, UnitAllocator, UnitId},
    FileSystem,
};
use directories::BaseDirs;
use failure::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::time::SystemTime;

#[macro_use]
mod macros;
mod copy_dir;
mod download_and_run;
mod git_sync;
mod install;
mod link;
mod link_dir;
mod only_for;

use self::copy_dir::CopyDir;
use self::download_and_run::DownloadAndRun;
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
                    $(
                    $name(ref system) => match system.translate() {
                        Translation::Keep => Translation::Keep,
                        Translation::Discard => Translation::Discard,
                        Translation::Expand(expanded) => Translation::Expand(expanded),
                    },
                    )*
                }
            }

            /// Get the id of this system.
            pub fn id(&self) -> Option<&str> {
                use self::System::*;

                match *self {
                    $($name(ref system) => system.id(),)*
                }
            }

            /// Get all things that this system depends on.
            pub fn requires(&self) -> &[String] {
                use self::System::*;

                match *self {
                    $($name(ref system) => system.requires(),)*
                }
            }

            /// Apply changes for this system.
            #[allow(unused)]
            pub fn apply<E>(&self, input: $crate::system::SystemInput<E>)
                -> Result<Vec<$crate::system::SystemUnit>, Error>
            where
                E: Copy + $crate::environment::Environment,
            {
                use failure::{ResultExt, format_err};
                use self::System::*;

                let res = match *self {
                    $($name(ref system) => system.apply(input),)*
                };

                Ok(res.with_context(|_| format_err!("Failed to run system: {:?}", self))?)
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
    #[serde(rename = "link")]
    Link(Link),
    #[serde(rename = "git-sync")]
    GitSync(GitSync),
    #[serde(rename = "only-for")]
    OnlyFor(OnlyFor),
}

system_impl![
    CopyDir,
    LinkDir,
    Install,
    DownloadAndRun,
    Link,
    GitSync,
    OnlyFor,
];

/// All inputs for a system.
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
    pub now: &'a SystemTime,
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
    ) -> impl IntoIterator<Item = unit::Dependency> {
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
                Dependency::Direct(id) => ids.push(unit::Dependency::Unit(id)),
                Dependency::None => continue,
            }
        }

        ids
    }
}
