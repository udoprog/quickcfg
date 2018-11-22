//! Packages abstraction for Debian.

use crate::{command, packages::Package};
use failure::{format_err, Error};
use std::ffi::OsStr;

#[derive(Debug)]
pub struct Apt {
    sudo: command::Command<'static>,
}

impl Apt {
    /// Create a new dpkg-query command wrapper.
    pub fn new() -> Self {
        Apt {
            sudo: command::Command::new("sudo"),
        }
    }

    /// List all the packages which are installed.
    pub fn install_packages<S>(&self, packages: impl IntoIterator<Item = S>) -> Result<(), Error>
    where
        S: AsRef<OsStr>,
    {
        let packages = packages.into_iter().collect::<Vec<_>>();

        let mut args = Vec::new();
        args.push(OsStr::new("-p"));
        args.push(OsStr::new("[sudo] password for %u to install packages: "));
        args.push(OsStr::new("--"));
        args.push(OsStr::new("apt"));
        args.push(OsStr::new("install"));
        args.push(OsStr::new("-y"));
        args.extend(packages.iter().map(AsRef::as_ref));

        self.sudo.run(args)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct DpkgQuery {
    dpkg_query: command::Command<'static>,
}

impl DpkgQuery {
    /// Create a new dpkg-query command wrapper.
    pub fn new() -> Self {
        DpkgQuery {
            dpkg_query: command::Command::new("dpkg-query"),
        }
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();

        let args = &["-W", "--showformat=${db:Status-Abbrev}${binary:Package}\\n"];

        for line in self.dpkg_query.run_lines(args)? {
            let line = line.trim();

            if line == "" {
                continue;
            }

            let mut it = line.split(" ");
            let status = it.next().ok_or_else(|| format_err!("expected status"))?;
            let name = it
                .next()
                .ok_or_else(|| format_err!("expected package name"))?;

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
pub struct Packages {
    dpkg_query: DpkgQuery,
    apt: Apt,
}

impl Packages {
    pub fn new() -> Self {
        Packages {
            dpkg_query: DpkgQuery::new(),
            apt: Apt::new(),
        }
    }

    /// List all packages on this system.
    pub fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.dpkg_query.list_installed()
    }

    /// List all the packages which are installed.
    pub fn install_packages<S>(&self, packages: impl IntoIterator<Item = S>) -> Result<(), Error>
    where
        S: AsRef<OsStr>,
    {
        self.apt.install_packages(packages)
    }
}