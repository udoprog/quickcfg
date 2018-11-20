use crate::{
    environment as e,
    system::SystemInput,
    template::Template,
    unit::{CopyFile, CreateDir, SystemUnit},
};
use failure::{bail, Error};
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

/// Builds one unit for every directory and file that needs to be copied.
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct CopyDir {
    /// Id of this system.
    pub id: Option<String>,
    /// Where to copy from.
    pub from: Template,
    /// Where to copy to.
    pub to: Template,
}

impl CopyDir {
    /// Access the ID of this system.
    pub fn id(&self) -> Option<String> {
        self.id.clone()
    }

    /// Copy one directory to another.
    pub fn apply<E>(self, input: SystemInput<E>) -> Result<Vec<SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        let SystemInput {
            root,
            facts,
            environment,
            allocator,
            ..
        } = input;

        let mut units = Vec::new();

        let from = match self.from.render_as_relative_path(facts, environment)? {
            Some(from) => from,
            None => return Ok(units),
        };

        let to = match self.to.render_as_relative_path(facts, environment)? {
            Some(to) => to,
            None => return Ok(units),
        };

        let from = from.to_path(root).canonicalize()?;
        let to = to.to_path(root).canonicalize()?;

        let mut parents = HashMap::new();

        for e in ignore::WalkBuilder::new(&from).hidden(false).build() {
            let e = e?;
            let s = e.path();
            let d = to.join(s.strip_prefix(&from)?);

            let s_m = s.metadata()?;
            let d_m = try_open_meta(&d)?;

            if s_m.is_dir() {
                if should_create_dir(d_m.as_ref())? {
                    let mut unit = allocator.unit(CreateDir(d.to_owned()));
                    parents.insert(d.to_owned(), unit.id());

                    if let Some(id) = d.parent().and_then(|p| parents.get(p)) {
                        unit.dependency(*id);
                    }

                    units.push(unit);
                }

                continue;
            }

            if s_m.is_file() {
                if should_copy_file(&s_m, d_m.as_ref())? {
                    let mut unit = allocator.unit(CopyFile(s.to_owned(), d.to_owned()));

                    if let Some(id) = d.parent().and_then(|p| parents.get(p)) {
                        unit.dependency(*id);
                    }

                    units.push(unit);
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
