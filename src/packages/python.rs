//! Packages abstraction for pip/pip3.

use crate::{command, os, packages::Package};
use anyhow::{anyhow, Error};
use std::ffi::OsStr;
use std::io;

#[derive(Debug)]
pub struct Pip {
    command: command::Command,
}

impl Pip {
    /// Create a new pip command wrapper.
    pub fn new(name: &'static str) -> Self {
        Pip {
            command: command::Command::new(os::command(name)),
        }
    }

    /// Test that the command is available.
    pub fn test(&self) -> Result<bool, Error> {
        let mut command = self.command.clone();
        command.arg("--version");

        match command.run() {
            Ok(output) => Ok(output.status.success()),
            Err(e) => match e.kind() {
                // no such command.
                io::ErrorKind::NotFound => Ok(false),
                _ => Err(Error::from(e)),
            },
        }
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();

        let mut command = self.command.clone();
        command.args(&["list", "--format=columns"]);

        for line in command.run_lines()? {
            let line = line.trim();

            if line == "" {
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

    /// List all the packages which are installed.
    pub fn install_packages<I>(&self, packages: I) -> Result<(), Error>
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        let mut command = self.command.clone();
        command.arg("install");
        command.arg("--user");
        command.args(packages);
        command.run()?;
        Ok(())
    }
}

/// Packages abstraction for pip.
#[derive(Debug)]
pub struct PackageManager {
    name: &'static str,
    pip: Pip,
}

impl PackageManager {
    /// Construct a new pip package manager.
    pub fn new(name: &'static str) -> Self {
        PackageManager {
            name,
            pip: Pip::new(name),
        }
    }
}

impl super::PackageManager for PackageManager {
    fn primary(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        self.name
    }

    /// Test if command is available.
    fn test(&self) -> Result<bool, Error> {
        self.pip.test()
    }

    fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.pip.list_installed()
    }

    fn install_packages(&self, packages: &[String]) -> Result<(), Error> {
        self.pip.install_packages(packages)
    }
}
