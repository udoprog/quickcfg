//! Packages abstraction for Ruby.

use crate::{command, os, packages::Package};
use anyhow::{anyhow, Error};
use std::ffi::OsStr;
use std::io;

#[derive(Debug)]
pub struct Gem {
    gem: command::Command<'static>,
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
        match self.gem.run(&["--version"]) {
            Ok(output) => Ok(output.status.success()),
            Err(e) => match e.kind() {
                // no such command.
                io::ErrorKind::NotFound => Ok(false),
                _ => Err(Error::from(e)),
            },
        }
    }

    /// List all the packages which are installed.
    pub fn install_packages<S>(&self, packages: impl IntoIterator<Item = S>) -> Result<(), Error>
    where
        S: AsRef<OsStr>,
    {
        let packages = packages.into_iter().collect::<Vec<_>>();

        let mut args = Vec::new();
        args.push(OsStr::new("install"));
        args.push(OsStr::new("--user-install"));
        args.extend(packages.iter().map(AsRef::as_ref));

        self.gem.run(args)?;
        Ok(())
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();

        for line in self.gem.run_lines(&["list", "-q", "-l"])? {
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
