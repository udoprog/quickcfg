use crate::{
    environment as e,
    system::SystemInput,
    unit::{self, SystemUnit},
};
use failure::{bail, Error};
use log::warn;
use serde_derive::Deserialize;
use std::collections::HashSet;

/// Builds one unit for every directory and file that needs to be copied.
system_struct! {
    InstallPackages {
        #[serde(default = "default_key")]
        pub key: String,
        pub provider: Option<String>,
    }
}

/// Default key to look up for installing packages.
fn default_key() -> String {
    String::from("packages")
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

        let (provider, packages) = match self.provider.as_ref() {
            Some(provider) => (Some(provider), packages.get(provider)),
            None => (None, packages.default()),
        };

        let packages = match packages {
            Some(packages) => packages,
            None => {
                if let Some(provider) = provider {
                    bail!("No package manager found for provider `{}`", provider);
                }

                warn!("No default package manager supported for system");
                return Ok(units);
            }
        };

        let mut to_install = HashSet::new();

        let key = match provider {
            Some(provider) => format!("{}::{}", provider, self.key),
            None => self.key.to_string(),
        };

        to_install.extend(data.load_or_default::<Vec<String>>(&key)?);

        for package in packages.list_packages()? {
            to_install.remove(&package.name);
        }

        if !to_install.is_empty() {
            let to_install = to_install.into_iter().collect();
            let mut unit = allocator.unit(unit::InstallPackages(to_install));
            unit.thread_local = true;
            units.push(unit);
        }

        return Ok(units);
    }
}
