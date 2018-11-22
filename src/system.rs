//! Things to do.

use crate::{
    environment as e,
    facts::Facts,
    file_utils::FileUtils,
    hierarchy::Data,
    packages,
    state::State,
    unit::{SystemUnit, UnitAllocator},
};
use directories::BaseDirs;
use failure::Error;
use serde_derive::Deserialize;
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
pub struct SystemInput<'a, E>
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
    pub state: &'a State,
}
