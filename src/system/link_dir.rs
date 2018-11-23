use crate::{
    environment as e, system::SystemInput, template::Template, unit::SystemUnit, FileUtils,
};
use failure::Error;
use serde_derive::Deserialize;

/// Builds one unit for every directory and entry that needs to be linked.
system_struct! {
    LinkDir {
        pub from: Template,
        pub to: Option<Template>,
    }
}

impl LinkDir {
    pub fn apply<E>(&self, input: SystemInput<E>) -> Result<Vec<SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        let SystemInput {
            root,
            base_dirs,
            facts,
            environment,
            file_utils,
            ..
        } = input;

        let mut units = Vec::new();

        let from = match self
            .from
            .render_as_path(root, base_dirs, facts, environment)?
        {
            Some(from) => from,
            None => return Ok(units),
        };

        // resolve destination, if unspecified defaults to relative current directory.
        let to = match self.to.as_ref() {
            Some(to) => match to.render_as_path(root, base_dirs, facts, environment)? {
                Some(to) => to,
                None => return Ok(units),
            },
            None => root.canonicalize()?,
        };

        for e in ignore::WalkBuilder::new(&from).hidden(false).build() {
            let e = e?;
            let from_path = e.path();
            let to_path = to.join(from_path.strip_prefix(&from)?);

            let from = from_path.symlink_metadata()?;
            let to = FileUtils::try_open_meta(&to_path)?;

            let source_type = from.file_type();

            if source_type.is_dir() {
                if FileUtils::should_create_dir(&to_path, to.as_ref())? {
                    units.extend(file_utils.create_dir_all(&to_path)?);
                }

                continue;
            }

            let link = match to_path
                .parent()
                .and_then(|p| FileUtils::path_relative_from(&from_path, p))
            {
                Some(link) => link,
                None => from_path.to_owned(),
            };

            // Maybe create a symlink!
            units.extend(file_utils.symlink(&to_path, link, to.as_ref())?);
        }

        return Ok(units);
    }
}
