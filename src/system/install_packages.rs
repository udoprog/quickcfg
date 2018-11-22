use crate::{
    environment as e,
    system::SystemInput,
    unit::{self, SystemUnit},
};
use failure::Error;
use serde_derive::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

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

        let mut to_install = HashSet::new();

        let key = match provider {
            Some(provider) => format!("{}::{}", provider, self.key),
            None => self.key.to_string(),
        };

        to_install.extend(data.load_or_default::<Vec<String>>(&key)?);

        let packages = match packages {
            Some(packages) => packages,
            None => {
                if !to_install.is_empty() {
                    return Ok(units);
                }

                // warn, because we have packages that we want to install but can't since there is
                // no package manager.
                match provider {
                    Some(provider) => {
                        log::warn!("No package manager for provider `{}` found", provider)
                    }
                    None => log::warn!("No primary package manager found"),
                }

                return Ok(units);
            }
        };

        for package in packages.list_packages()? {
            to_install.remove(&package.name);
        }

        if !to_install.is_empty() {
            let to_install = to_install.into_iter().collect();
            let mut unit = allocator.unit(unit::InstallPackages(Arc::clone(packages), to_install));
            unit.thread_local = true;
            units.push(unit);
        }

        return Ok(units);
    }
}
