use crate::{
    environment as e, system::SystemInput, template::Template, unit::SystemUnit, FileUtils,
};
use failure::{bail, Error};
use serde_derive::Deserialize;
use std::fs;
use std::path::Path;

/// Builds one unit for every directory and file that needs to be copied.
system_struct! {
    CopyDir {
        pub from: Template,
        pub to: Option<Template>,
        #[serde(default)]
        pub templates: bool,
    }
}

impl CopyDir {
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

            // NB: do not copy link.
            if source_type.is_symlink() {
                let link = fs::read_link(from_path)?;

                if FileUtils::should_create_symlink(&to_path, &link, to.as_ref())? {
                    units.push(file_utils.symlink(&to_path, link)?);
                }

                continue;
            }

            if source_type.is_dir() {
                if should_create_dir(&to_path, to.as_ref())? {
                    units.extend(file_utils.create_dir_all(&to_path)?);
                }

                continue;
            }

            if source_type.is_file() {
                if should_copy_file(&from, &to_path, to.as_ref())? {
                    units.push(file_utils.copy_file(&from_path, &to_path, self.templates)?);
                }

                continue;
            }

            bail!(
                "cannot handle file with metadata `{:?}`: {}",
                from,
                from_path.display()
            );
        }

        return Ok(units);

        /// Test if we should create the destination directory.
        ///
        /// Pretty straight forward: if it doesn't exist then YES.
        fn should_create_dir(path: &Path, meta: Option<&fs::Metadata>) -> Result<bool, Error> {
            let meta = match meta {
                Some(meta) => meta,
                None => return Ok(true),
            };

            let ty = meta.file_type();

            if !ty.is_dir() {
                bail!("Exists but is not a dir: {}", path.display());
            }

            Ok(false)
        }

        /// Test if we should copy the file.
        ///
        /// This is true if:
        ///
        /// * The destination file does not exist.
        /// * The destination file has a modified timestamp less than the source file.
        fn should_copy_file(
            from: &fs::Metadata,
            to_path: &Path,
            to: Option<&fs::Metadata>,
        ) -> Result<bool, Error> {
            let to = match to {
                Some(to) => to,
                None => return Ok(true),
            };

            if !to.is_file() {
                bail!("Exists but is not a file: {}", to_path.display());
            }

            Ok(from.modified()? > to.modified()?)
        }
    }
}
