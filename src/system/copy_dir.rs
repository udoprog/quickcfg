use crate::{
    environment as e, system::SystemInput, template::Template, unit::SystemUnit, FileSystem,
};
use anyhow::{bail, Error};
use std::fmt;
use std::fs;

system_struct! {
    #[doc = "Builds one unit for every directory and file that needs to be copied."]
    CopyDir {
        #[doc="Where to copy from."]
        pub from: Template,
        #[doc="Where to copy to."]
        pub to: Template,
        #[serde(default)]
        #[doc="If we should treat files as templates."]
        pub templates: bool,
    }
}

impl CopyDir {
    system_defaults!(translate);

    /// Copy one directory to another.
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

            if source_type.is_symlink() {
                let link = fs::read_link(from_path)?;
                units.extend(file_system.symlink(&to_path, link, to.as_ref())?);
                continue;
            }

            if source_type.is_dir() {
                if FileSystem::should_create_dir(&to_path, to.as_ref())? {
                    units.extend(file_system.create_dir_all(&to_path)?);
                }

                continue;
            }

            if source_type.is_file() {
                units.extend(file_system.copy_file(
                    &from_path,
                    from,
                    &to_path,
                    to.as_ref(),
                    self.templates,
                )?);
                continue;
            }

            bail!(
                "Cannot handle file with metadata `{:?}`: {}",
                from,
                from_path.display()
            );
        }

        Ok(units)
    }
}

impl fmt::Display for CopyDir {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "copy directory from `{}` to `{}`", self.from, self.to)
    }
}
