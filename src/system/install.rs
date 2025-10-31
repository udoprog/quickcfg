use crate::{
    environment as e,
    system::SystemInput,
    unit::{self, SystemUnit},
};
use anyhow::{Error, anyhow};
use std::collections::{BTreeSet, HashSet};
use std::fmt;

system_struct! {
    #[doc = "Builds one unit for every batch of packages to install."]
    Install {
        #[doc="Hierarchy key to lookup for packages to install."]
        #[serde(default = "default_key")]
        pub key: String,
        #[doc="Package provider to use."]
        pub provider: Option<String>,
    }
}

/// Default key to look up for installing packages.
fn default_key() -> String {
    String::from("packages")
}

impl Install {
    system_defaults!(translate);

    /// Copy one directory to another.
    pub fn apply<E>(&self, input: SystemInput<E>) -> Result<Vec<SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        let SystemInput {
            packages,
            data,
            allocator,
            state,
            ..
        } = input;

        let mut units = Vec::new();

        let provider = self.provider.as_deref();

        let package_manager = match provider {
            Some(provider) => packages.get(provider)?,
            None => packages.default(),
        };

        let id = self
            .id
            .as_deref()
            .map(|id| id.to_string())
            .or_else(|| provider.map(|id| id.to_string()))
            .or_else(|| packages.default().map(|p| p.name().to_string()))
            .ok_or_else(|| anyhow!("no usable install provider id"))?;

        let mut all_packages = BTreeSet::new();

        let key = match package_manager.as_deref().and_then(|p| p.key()) {
            Some(key) => key.to_string(),
            None => match provider {
                Some(provider) => format!("{}::{}", provider, self.key),
                None => self.key.to_string(),
            },
        };

        all_packages.extend(data.load_first_or_default::<Vec<String>>(&key)?);

        // test if stored hash is stale.
        if state.is_hash_fresh(&id, &all_packages)? {
            log::trace!("Skipping `{id}` since hash is fresh");
            return Ok(units);
        }

        let package_manager = match package_manager {
            Some(package_manager) => package_manager,
            None => {
                if !all_packages.is_empty() {
                    return Ok(units);
                }

                // warn, because we have packages that we want to install but can't since there is
                // no package manager.
                match provider {
                    Some(provider) => {
                        log::warn!("No package manager for provider `{provider}` found")
                    }
                    None => log::warn!("No primary package manager found"),
                }

                return Ok(units);
            }
        };

        let mut to_install = all_packages.iter().cloned().collect::<HashSet<_>>();

        for package in package_manager.list_packages()? {
            to_install.remove(&package.name);
        }

        let to_install = to_install.into_iter().collect();

        // thread-local if package manager requires user interaction.
        let thread_local = package_manager.needs_interaction();

        let mut unit = allocator.unit(unit::Install {
            package_manager,
            all_packages,
            to_install,
            id,
        });

        // NB: sometimes requires user input.
        unit.thread_local = thread_local;
        units.push(unit);
        Ok(units)
    }
}

impl fmt::Display for Install {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.provider.as_ref() {
            Some(provider) => write!(fmt, "install packages using provider `{provider}`"),
            None => write!(fmt, "install packages using primary provider"),
        }
    }
}
