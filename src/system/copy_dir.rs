use crate::{environment as e, system::SystemInput, template::Template, unit::SystemUnit};
use failure::{bail, format_err, Error};
use relative_path::RelativePathBuf;
use serde_derive::Deserialize;
use std::fs;
use std::io;
use std::path::Path;

/// Builds one unit for every directory and file that needs to be copied.
system_struct! {
    CopyDir {
        pub from: Template,
        pub to: Option<Template>,
        #[serde(default)]
        pub to_home: bool,
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

        let relative_from = match self.from.render_as_relative_path(facts, environment)? {
            Some(from) => from,
            None => return Ok(units),
        };

        let from = relative_from.to_path(root).canonicalize()?;

        // resolve destination, if unspecified defaults to relative current directory.
        let relative_to = match self.to.as_ref() {
            Some(to) => match to.render_as_relative_path(facts, environment)? {
                Some(to) => to,
                None => return Ok(units),
            },
            None => RelativePathBuf::from("."),
        };

        let to = {
            if self.to_home {
                let base_dirs =
                    base_dirs.ok_or_else(|| format_err!("no base directories available"))?;
                relative_to.to_path(base_dirs.home_dir()).canonicalize()?
            } else {
                relative_to.to_path(root).canonicalize()?
            }
        };

        for e in ignore::WalkBuilder::new(&from).hidden(false).build() {
            let e = e?;
            let s = e.path();
            let d = to.join(s.strip_prefix(&from)?);

            let s_m = s.metadata()?;
            let d_m = try_open_meta(&d)?;

            if s_m.is_dir() {
                if should_create_dir(d_m.as_ref())? {
                    units.extend(file_utils.create_dir_all(&d)?);
                }

                continue;
            }

            if s_m.is_file() {
                if should_copy_file(&s_m, d_m.as_ref())? {
                    units.push(file_utils.copy_file(&s, &d)?);
                }

                continue;
            }

            bail!(
                "cannot handle file with metadata `{:?}`: {}",
                s_m,
                s.display()
            );
        }

        return Ok(units);

        /// Try to open metadata, unless the file does not exist.
        ///
        /// If the file does not exist, returns `None`.
        fn try_open_meta(p: &Path) -> Result<Option<fs::Metadata>, Error> {
            match p.metadata() {
                Ok(m) => Ok(Some(m)),
                Err(e) => match e.kind() {
                    io::ErrorKind::NotFound => Ok(None),
                    _ => bail!("to get metadata: {}: {}", p.display(), e),
                },
            }
        }

        /// Test if we should create the destination directory.
        ///
        /// Pretty straight forward: if it doesn't exist then YES.
        fn should_create_dir(d: Option<&fs::Metadata>) -> Result<bool, Error> {
            Ok(d.is_none())
        }

        /// Test if we should copy the file.
        ///
        /// This is true if:
        ///
        /// * The destination file does not exist.
        /// * The destination file has a modified timestamp less than the source file.
        fn should_copy_file(s: &fs::Metadata, d: Option<&fs::Metadata>) -> Result<bool, Error> {
            let d = match d {
                Some(d) => d,
                None => return Ok(true),
            };

            if !d.is_file() {
                return Ok(true);
            }

            Ok(s.modified()? > d.modified()?)
        }
    }
}
