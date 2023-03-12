//! Packages abstraction for Fedora.

use crate::{command, os, packages::Package};
use anyhow::{anyhow, Error};
use std::ffi::OsStr;
use std::io;

#[derive(Debug)]
pub struct Dnf {
    sudo: command::Command,
    dnf: command::Command,
}

impl Dnf {
    /// Create a new dpkg-query command wrapper.
    pub fn new() -> Self {
        Dnf {
            sudo: command::Command::new(os::command("sudo")),
            dnf: command::Command::new(os::command("dnf")),
        }
    }

    /// Test that the command is available.
    pub fn test(&self) -> Result<bool, Error> {
        let mut dnf = self.dnf.clone();
        dnf.arg("--version");

        match dnf.run() {
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
        sudo.args(&["dnf", "install", "-y"]);
        sudo.args(packages);
        sudo.run_inherited()?;
        Ok(())
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();

        let mut dnf = self.dnf.clone();
        dnf.args(&["list", "--installed"]);

        for line in dnf.run_lines()?.into_iter().skip(1) {
            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            let mut it = line.split(' ');
            let name = it.next().ok_or_else(|| anyhow!("expected package name"))?;

            let name = name
                .split_once('.')
                .ok_or_else(|| anyhow!("illegal name"))?
                .0;

            out.push(Package {
                name: name.to_string(),
            });
        }

        Ok(out)
    }
}

/// Packages abstraction for Fedora.
#[derive(Debug)]
pub struct PackageManager {
    dnf: Dnf,
}

impl PackageManager {
    /// Construct a new dnf package manager.
    pub fn new() -> Self {
        PackageManager { dnf: Dnf::new() }
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
        "fedora"
    }

    /// Test that we have everything we need.
    fn test(&self) -> Result<bool, Error> {
        self.dnf.test()
    }

    fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.dnf.list_installed()
    }

    fn install_packages(&self, packages: &[String]) -> Result<(), Error> {
        self.dnf.install_packages(packages)
    }
}
