use crate::{
    environment as e, system::SystemInput, template::Template, unit::SystemUnit, FileSystem,
};
use anyhow::Error;
use std::fmt;

system_struct! {
    #[doc = "Recursively creates directories and copies files."]
    LinkDir {
        #[doc="Where to link files from."]
        pub from: Template,
        #[doc="Where to link files to."]
        pub to: Template,
    }
}

impl LinkDir {
    system_defaults!(translate);

    pub fn apply<E>(&self, input: SystemInput<E>) -> Result<Vec<SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        let SystemInput {
            root,
            base_dirs,
            facts,
            environment,
            file_system,
            ..
        } = input;

        let mut units = Vec::new();

        let from = match self.from.as_path(root, base_dirs, facts, environment)? {
            Some(from) => from,
            None => return Ok(units),
        };

        // resolve destination, if unspecified defaults to relative current directory.
        let to = match self.to.as_path(root, base_dirs, facts, environment)? {
            Some(to) => to,
            None => return Ok(units),
        };

        for e in ignore::WalkBuilder::new(&from).hidden(false).build() {
            let e = e?;
            let from_path = e.path();
            let to_path = to.join(from_path.strip_prefix(&from)?);

            let from = from_path.symlink_metadata()?;
            let to = FileSystem::try_open_meta(&to_path)?;

            let source_type = from.file_type();

            if source_type.is_dir() {
                if FileSystem::should_create_dir(&to_path, to.as_ref())? {
                    units.extend(file_system.create_dir_all(&to_path)?);
                }

                continue;
            }

            let link = to_path
                .parent()
                .and_then(|p| FileSystem::path_relative_from(from_path, p))
                .unwrap_or_else(|| from_path.to_owned());

            // Maybe create a symlink!
            units.extend(file_system.symlink(&to_path, link, to.as_ref())?);
        }

        Ok(units)
    }
}

impl fmt::Display for LinkDir {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "link directory `{}` to `{}`", self.from, self.to)
    }
}
