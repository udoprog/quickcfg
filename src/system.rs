//! Things to do.

use crate::{
    environment as e,
    facts::Facts,
    file_utils::FileUtils,
    hierarchy::Data,
    opts::Opts,
    packages,
    state::State,
    unit::{self, SystemUnit, UnitAllocator, UnitId},
};
use directories::BaseDirs;
use failure::Error;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

mod copy_dir;
mod download_and_run;
mod git_sync;
mod install_packages;
mod link;
mod link_dir;

use self::copy_dir::CopyDir;
use self::download_and_run::DownloadAndRun;
use self::git_sync::GitSync;
use self::install_packages::InstallPackages;
use self::link::Link;
use self::link_dir::LinkDir;

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum System {
    #[serde(rename = "copy-dir")]
    CopyDir(CopyDir),
    #[serde(rename = "link-dir")]
    LinkDir(LinkDir),
    #[serde(rename = "install-packages")]
    InstallPackages(InstallPackages),
    #[serde(rename = "download-and-run")]
    DownloadAndRun(DownloadAndRun),
    #[serde(rename = "link")]
    Link(Link),
    #[serde(rename = "git-sync")]
    GitSync(GitSync),
}

impl System {
    system_functions![
        CopyDir,
        LinkDir,
        InstallPackages,
        DownloadAndRun,
        Link,
        GitSync,
    ];
}

/// All inputs for a system.
#[derive(Clone)]
pub struct SystemInput<'a, 'state: 'a, E>
where
    E: Copy + e::Environment,
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
    pub file_utils: &'a FileUtils<'a>,
    /// State accessor.
    pub state: &'a State<'state>,
    /// Current time.
    pub now: &'a SystemTime,
    /// Current optsion.
    pub opts: &'a Opts,
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
