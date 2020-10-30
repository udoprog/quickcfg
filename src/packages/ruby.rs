//! Packages abstraction for Ruby.

use crate::{command, os, packages::Package};
use anyhow::{anyhow, Error};
use std::ffi::OsStr;
use std::io;

#[derive(Debug)]
pub struct Gem {
    gem: command::Command,
}

impl Gem {
    /// Create a new gem command wrapper.
    pub fn new() -> Self {
        Gem {
            gem: command::Command::new(os::command("gem")),
        }
    }

    /// Test that the command is available.
    pub fn test(&self) -> Result<bool, Error> {
        let mut gem = self.gem.clone();
        gem.arg("--version");

        match gem.run() {
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
        let mut gem = self.gem.clone();
        gem.arg("install");
        gem.arg("--user-install");
        gem.args(packages);
        gem.run()?;
        Ok(())
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();
        let mut gem = self.gem.clone();
        gem.args(&["list", "-q", "-l"]);

        for line in gem.run_lines()? {
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
}

/// Packages abstraction for Ruby.
#[derive(Debug)]
pub struct PackageManager {
    gem: Gem,
}

impl PackageManager {
    /// Construct a new ruby package manager.
    pub fn new() -> Self {
        PackageManager { gem: Gem::new() }
    }
}

impl super::PackageManager for PackageManager {
    fn primary(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "gem"
    }

    /// Test that we have everything we need.
    fn test(&self) -> Result<bool, Error> {
        self.gem.test()
    }

    fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.gem.list_installed()
    }

    fn install_packages(&self, packages: &[String]) -> Result<(), Error> {
        self.gem.install_packages(packages)
    }
}
