//! Packages abstraction for Debian.

use crate::{command, os, packages::Package};
use anyhow::{Error, anyhow};
use std::ffi::OsStr;
use std::io;

#[derive(Debug)]
pub struct Apt {
    sudo: command::Command,
    apt: command::Command,
}

impl Apt {
    /// Create a new dpkg-query command wrapper.
    pub fn new() -> Self {
        Apt {
            sudo: command::Command::new(os::command("sudo")),
            apt: command::Command::new(os::command("apt")),
        }
    }

    /// Test that the command is available.
    pub fn test(&self) -> Result<bool, Error> {
        let mut apt = self.apt.clone();
        apt.arg("--version");

        match apt.run() {
            Ok(output) => Ok(output.status.success()),
            Err(e) => match e.kind() {
                // no such command.
                io::ErrorKind::NotFound => Ok(false),
                _ => Err(Error::from(e)),
            },
        }
    }

    /// List all the packages which are installed.
    pub fn install_packages<I>(&self, packages: I) -> Result<(), Error>
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        let mut sudo = self.sudo.clone();
        sudo.args(&["-p", "[sudo] password for %u to install packages: ", "--"]);
        sudo.args(&["apt", "install", "-y"]);
        sudo.args(packages);
        sudo.run_inherited()?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct DpkgQuery {
    dpkg_query: command::Command,
}

impl DpkgQuery {
    /// Create a new dpkg-query command wrapper.
    pub fn new() -> Self {
        DpkgQuery {
            dpkg_query: command::Command::new(os::command("dpkg-query")),
        }
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();

        let mut dpkg_query = self.dpkg_query.clone();
        dpkg_query.args(&["-W", "--showformat=${db:Status-Abbrev}${binary:Package}\\n"]);

        for line in dpkg_query.run_lines()? {
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            let mut it = line.split(' ');
            let status = it.next().ok_or_else(|| anyhow!("expected status"))?;
            let name = it.next().ok_or_else(|| anyhow!("expected package name"))?;

            if status != "ii" {
                continue;
            }

            out.push(Package {
                name: name.to_string(),
            });
        }

        Ok(out)
    }
}

/// Packages abstraction for Debian.
#[derive(Debug)]
pub struct PackageManager {
    dpkg_query: DpkgQuery,
    apt: Apt,
}

impl PackageManager {
    /// Construct a new debian package manager.
    pub fn new() -> Self {
        PackageManager {
            dpkg_query: DpkgQuery::new(),
            apt: Apt::new(),
        }
    }
}

impl super::PackageManager for PackageManager {
    fn primary(&self) -> bool {
        true
    }

    fn needs_interaction(&self) -> bool {
        // needs interaction because we use `sudo`.
        true
    }

    fn name(&self) -> &str {
        "debian"
    }

    /// Test that we have everything we need.
    fn test(&self) -> Result<bool, Error> {
        self.apt.test()
    }

    fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.dpkg_query.list_installed()
    }

    fn install_packages(&self, packages: &[String]) -> Result<(), Error> {
        self.apt.install_packages(packages)
    }
}
