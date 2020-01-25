use crate::{
    environment as e, system::SystemInput, template::Template, unit::SystemUnit, FileSystem,
};
use anyhow::Error;
use std::fmt;

system_struct! {
    #[doc = "Builds one unit for every directory and file that needs to be copied."]
    Link {
        #[doc="Where to create the symlink."]
        pub path: Template,
        #[doc="Where to point the created symlink."]
        pub link: Template,
    }
}

impl Link {
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

        let path = match self.path.as_path(root, base_dirs, facts, environment)? {
            Some(path) => path,
            None => return Ok(units),
        };

        let link = match self.link.as_path(root, base_dirs, facts, environment)? {
            Some(link) => link,
            None => return Ok(units),
        };

        let m = FileSystem::try_open_meta(&path)?;

        // try to relativize link.
        let link = if link.is_absolute() {
            path.parent()
                .and_then(|p| FileSystem::path_relative_from(&link, p))
                .unwrap_or_else(|| link)
        } else {
            link
        };

        units.extend(file_system.symlink(&path, link, m.as_ref())?);
        Ok(units)
    }
}

impl fmt::Display for Link {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "link `{}` to `{}`", self.path, self.link)
    }
}
