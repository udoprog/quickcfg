//! Packages abstraction for Cargo.

use crate::{command, os, packages::Package};
use anyhow::{Error, anyhow};
use std::ffi::OsStr;
use std::io;

#[derive(Debug)]
pub struct Cargo {
    cargo: command::Command,
}

impl Cargo {
    /// Create a new cargo command wrapper.
    pub fn new() -> Self {
        Cargo {
            cargo: command::Command::new(os::command("cargo")),
        }
    }

    /// Test that the command is available.
    pub fn test(&self) -> Result<bool, Error> {
        let mut cargo = self.cargo.clone();
        cargo.arg("--version");

        match cargo.run() {
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
        let mut cargo = self.cargo.clone();
        cargo.arg("install");
        cargo.args(packages);
        cargo.run()?;
        Ok(())
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();

        let mut cargo = self.cargo.clone();
        cargo.args(&["install", "--list"]);

        for line in cargo.run_lines()? {
            if line.starts_with(char::is_whitespace) {
                continue;
            }

            let line = line.trim();

            if line.is_empty() {
                continue;
            }

            let mut it = line.split(' ');

            let name = it.next().ok_or_else(|| anyhow!("expected package name"))?;

            out.push(Package {
                name: name.to_string(),
            });
        }

        Ok(out)
    }
}

/// Packages abstraction for Cargo.
#[derive(Debug)]
pub struct PackageManager {
    cargo: Cargo,
}

impl PackageManager {
    /// Construct a new cargo package manager.
    pub fn new() -> Self {
        PackageManager {
            cargo: Cargo::new(),
        }
    }
}

impl super::PackageManager for PackageManager {
    fn primary(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "cargo"
    }

    /// Test that we have everything we need.
    fn test(&self) -> Result<bool, Error> {
        self.cargo.test()
    }

    fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.cargo.list_installed()
    }

    fn install_packages(&self, packages: &[String]) -> Result<(), Error> {
        self.cargo.install_packages(packages)
    }
}
