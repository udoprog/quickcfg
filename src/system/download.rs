use crate::{
    environment as e,
    system::SystemInput,
    template::Template,
    unit::{self, Dependency},
};
use anyhow::{anyhow, bail, Context as _, Error};
use std::fmt;

system_struct! {
    #[doc = "Builds one unit for every directory and file that needs to be copied."]
    Download {
        #[doc="URL to download."]
        pub url: String,
        #[doc="Where to download the file to."]
        pub path: Template,
    }
}

impl Download {
    system_defaults!(translate);

    /// Copy one directory to another.
    pub fn apply<E>(&self, input: SystemInput<E>) -> Result<Vec<unit::SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        let SystemInput {
            root,
            base_dirs,
            allocator,
            state,
            facts,
            environment,
            file_system,
            ..
        } = input;

        let url = reqwest::Url::parse(&self.url).with_context(|| anyhow!("illegal `url`"))?;
        let base = url_base_name(&url);

        let generated_id;

        let id = {
            if let Some(base) = base {
                generated_id = format!("{id}-{base}", id = id_from_url(&self.url), base = base);
            } else {
                generated_id = id_from_url(&self.url);
            }

            generated_id.as_str()
        };

        if state.has_run_once(id) {
            return Ok(vec![]);
        }

        let path = match self.path.as_path(root, base_dirs, facts, environment)? {
            Some(path) => path,
            None => bail!("target path is not supported"),
        };

        let mut units = Vec::new();
        let mut create_dirs = Vec::new();

        if let Some(parent) = path.parent() {
            create_dirs.extend(file_system.create_dir_all(parent)?);
        }

        let mut download = allocator.unit(unit::Download {
            url,
            path,
            id: None,
        });

        download
            .dependencies
            .extend(create_dirs.iter().map(|u| Dependency::Dir(u.id)));

        units.extend(create_dirs);
        units.push(download);

        Ok(units)
    }
}

impl fmt::Display for Download {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "download `{}`", self.url)
    }
}

/// Generate a unique ID from the URL provided.
fn id_from_url(url: &str) -> String {
    use std::hash::{Hash, Hasher};

    let mut state = fxhash::FxHasher64::default();
    url.hash(&mut state);

    format!("{:x}", state.finish())
}

/// Extract a reasonable URL base name.
fn url_base_name(url: &reqwest::Url) -> Option<&str> {
    let base = url.path().rsplit('/').next()?;

    if base.is_empty() {
        return None;
    }

    Some(base)
}
