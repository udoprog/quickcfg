//! Packages abstraction for Debian.

use failure::{format_err, Error};
use crate::{
    packages::Package,
    command,
};

#[derive(Debug)]
pub struct DpkgQuery(command::Command<'static>);

impl DpkgQuery {
    /// Create a new dpkg-query command wrapper.
    pub fn new() -> Self {
        DpkgQuery(command::Command::new("dpkg-query"))
    }

    /// List all the packages which are installed.
    pub fn list_installed(&self) -> Result<Vec<Package>, Error> {
        let mut out = Vec::new();

        let args = &["-W", "--showformat=${db:Status-Abbrev}${binary:Package}\\n"];

        for line in self.0.run_lines(args)? {
            let line = line.trim();

            if line == "" {
                continue;
            }

            let mut it = line.split(" ");
            let status = it.next().ok_or_else(|| format_err!("expected status"))?;
            let name = it.next().ok_or_else(|| format_err!("expected package name"))?;

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
}

impl Packages {
    pub fn new() -> Self {
        Packages {
            dpkg_query: DpkgQuery::new(),
        }
    }

    /// List all packages on this system.
    pub fn list_packages(&self) -> Result<Vec<Package>, Error> {
        self.dpkg_query.list_installed()
    }
}
