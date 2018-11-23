use crate::{
    config, environment as e,
    git::Git,
    system::SystemInput,
    template::Template,
    unit::{GitClone, GitUpdate, SystemUnit},
};
use failure::{format_err, Error};
use serde_derive::Deserialize;
use std::time::Duration;

const DEFAULT_REFRESH: u64 = 3600 * 24;

/// Checkout a git repository to the given path.
system_struct! {
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
    /// Copy one directory to another.
    pub fn apply<E>(&self, input: SystemInput<E>) -> Result<Vec<SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        let SystemInput {
            root,
            base_dirs,
            allocator,
            file_utils,
            state,
            facts,
            environment,
            now,
            opts,
            ..
        } = input;

        let id = self
            .id
            .as_ref()
            .ok_or_else(|| format_err!("missing `id`"))?;

        let id = format!("git-sync/{}", id);

        let mut units = Vec::new();

        let path = match self.path.as_path(root, base_dirs, facts, environment)? {
            Some(path) => path,
            None => return Ok(units),
        };

        if let Some(last_update) = state.last_update(&id) {
            let duration = now.duration_since(last_update.clone())?;

            if duration < self.refresh {
                return Ok(units);
            }
        };

        let git = Git::new(path.as_path());

        if !git.path.is_dir() {
            let create_dirs = match git.path.parent() {
                Some(parent) => file_utils.create_dir_all(parent)?,
                None => vec![],
            };

            let mut git_clone = allocator.unit(GitClone {
                id,
                git,
                remote: self.remote.to_string(),
            });

            git_clone.thread_local = true;

            git_clone
                .dependencies
                .extend(create_dirs.iter().map(|u| u.id));

            units.extend(create_dirs);
            units.push(git_clone);
            return Ok(units);
        }

        if !git.test()? {
            log::warn!("no working git command found");
            return Ok(units);
        }

        let mut git_update = allocator.unit(GitUpdate {
            id,
            git,
            force: opts.force,
        });

        git_update.thread_local = true;
        units.push(git_update);
        return Ok(units);
    }
}
