use crate::{
    config, environment as e,
    system::SystemInput,
    template::Template,
    unit::{GitClone, GitUpdate, SystemUnit},
};
use anyhow::{Error, anyhow};
use std::fmt;
use std::time::Duration;

const DEFAULT_REFRESH: u64 = 3600 * 24;

system_struct! {
    #[doc = "Checkout a git repository to the given path."]
    GitSync {
        #[doc="Path to check out the repo."]
        pub path: Template,
        #[doc="Remote to keep in sync with."]
        pub remote: String,
        #[serde(
            default = "default_refresh",
            deserialize_with = "config::human_duration"
        )]
        pub refresh: Duration,
    }
}

/// Get default refresh.
fn default_refresh() -> Duration {
    Duration::from_secs(DEFAULT_REFRESH)
}

impl GitSync {
    system_defaults!(translate);

    /// Copy one directory to another.
    pub fn apply<E>(&self, input: SystemInput<E>) -> Result<Vec<SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        let SystemInput {
            root,
            base_dirs,
            allocator,
            file_system,
            state,
            facts,
            environment,
            now,
            opts,
            git_system,
            ..
        } = input;

        let id = self.id.as_ref().ok_or_else(|| anyhow!("missing `id`"))?;

        let id = format!("git-sync/{id}");

        let mut units = Vec::new();

        let path = match self.path.as_path(root, base_dirs, facts, environment)? {
            Some(path) => path,
            None => return Ok(units),
        };

        if let Some(last_update) = state.last_update(&id) {
            let duration = now.duration_since(*last_update)?;

            if duration < self.refresh {
                return Ok(units);
            }
        };

        if !git_system.test()? {
            log::warn!("no working git command found");
            return Ok(units);
        }

        if path.is_dir() {
            let git_update = allocator.unit(GitUpdate {
                id,
                path,
                force: opts.force,
            });

            units.push(git_update);
            return Ok(units);
        }

        // Initial clone.
        let parent_dir = match path.parent() {
            Some(parent) if !parent.is_dir() => {
                units.extend(file_system.create_dir_all(parent)?);
                Some(file_system.dir_dependency(parent)?)
            }
            _ => None,
        };

        let dir_dependencies = file_system.dir_dependency(&path)?;

        let mut git_clone = allocator.unit(GitClone {
            id,
            path,
            remote: self.remote.to_string(),
        });

        git_clone.dependencies.extend(parent_dir);
        git_clone.provides.push(dir_dependencies);

        units.push(git_clone);
        Ok(units)
    }
}

impl fmt::Display for GitSync {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "syncing remote `{}` to `{}`", self.remote, self.path)
    }
}
