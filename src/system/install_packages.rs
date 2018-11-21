use crate::{
    environment as e,
    system::SystemInput,
    unit::{self, SystemUnit},
};
use log::warn;
use failure::Error;
use serde_derive::Deserialize;
use std::collections::HashSet;

/// Builds one unit for every directory and file that needs to be copied.
system_struct! {
    InstallPackages {
        pub key: Option<String>,
    }
}

impl InstallPackages {
    /// Copy one directory to another.
    pub fn apply<E>(&self, input: SystemInput<E>) -> Result<Vec<SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        let SystemInput {
            packages,
            data,
            allocator,
            ..
        } = input;

        let mut units = Vec::new();

        let packages = match packages {
            Some(packages) => packages,
            None => {
                warn!("Cannot execute system, not package manager detected");
                return Ok(units);
            }
        };

        let mut packages_to_install = HashSet::new();

        if let Some(key) = self.key.as_ref() {
            let packages = data.load_or_default::<Vec<String>>(key)?;
            packages_to_install.extend(packages);
        };

        for package in packages.list_packages()? {
            packages_to_install.remove(&package.name);
        }

        if !packages_to_install.is_empty() {
            units.push(allocator.unit(unit::InstallPackages(packages_to_install)));
        }

        return Ok(units);
    }
}
