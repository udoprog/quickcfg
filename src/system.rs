//! Things to do.

use crate::{
    environment as e,
    facts::Facts,
    file_utils::FileUtils,
    hierarchy::Data,
    packages,
    state::State,
    unit::{SystemUnit, UnitAllocator, UnitId},
};
use directories::BaseDirs;
use failure::Error;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::path::Path;

mod copy_dir;
mod download_and_run;
mod install_packages;

use self::copy_dir::CopyDir;
use self::download_and_run::DownloadAndRun;
use self::install_packages::InstallPackages;

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum System {
    #[serde(rename = "copy-dir")]
    CopyDir(CopyDir),
    #[serde(rename = "install-packages")]
    InstallPackages(InstallPackages),
    #[serde(rename = "download-and-run")]
    DownloadAndRun(DownloadAndRun),
}

impl System {
    system_functions![CopyDir, InstallPackages, DownloadAndRun,];
}

/// All inputs for a system.
#[derive(Clone)]
pub struct SystemInput<'a, 'c: 'a, E>
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
    pub state: &'a State<'c>,
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
    ) -> impl IntoIterator<Item = UnitId> {
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
                Dependency::Direct(id) => ids.push(id),
                Dependency::None => continue,
            }
        }

        ids
    }
}
