use crate::{
    environment as e, os,
    system::SystemInput,
    template::Template,
    unit::{AddMode, Dependency, Download, Mode, RunOnce, SystemUnit},
};
use anyhow::{anyhow, Context as _, Error};
use std::fmt;

system_struct! {
    #[doc = "Builds one unit for every directory and file that needs to be copied."]
    DownloadAndRun {
        #[doc="URL to download."]
        pub url: String,
        #[doc="Run the command through `/bin/sh`."]
        #[serde(default)]
        pub shell: bool,
        #[doc="Does the command require interaction."]
        #[serde(default)]
        pub interactive: bool,
        #[doc="Arguments to add when running command."]
        #[serde(default)]
        pub args: Vec<Template>,
        #[doc="Rename the binary to this before running it."]
        #[serde(default)]
        pub name: Option<String>,
        /// Run the downloaded command as root.
        #[serde(default)]
        pub root: bool,
    }
}

impl DownloadAndRun {
    system_defaults!(translate);

    /// Copy one directory to another.
    pub fn apply<E>(&self, input: SystemInput<E>) -> Result<Vec<SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        let SystemInput {
            allocator,
            file_system,
            state,
            facts,
            environment,
            ..
        } = input;

        let url = reqwest::Url::parse(&self.url).with_context(|| anyhow!("illegal `url`"))?;
        let base = url_base_name(&url);

        let generated_id;

        let id = match self.id.as_deref().or_else(|| self.name.as_deref()) {
            Some(id) => id,
            None => {
                if let Some(base) = base {
                    generated_id = format!("{id}-{base}", id = id_from_url(&self.url), base = base);
                } else {
                    generated_id = id_from_url(&self.url);
                }

                generated_id.as_str()
            }
        };

        if state.has_run_once(id) {
            return Ok(vec![]);
        }

        let name = if let Some(name) = self.name.as_deref() {
            name
        } else {
            id
        };

        let path = os::exe_path(file_system.state_path(name));

        let mut units = Vec::new();

        let download = if !path.is_file() {
            // Download the file.
            Some(allocator.unit(Download(url, path.to_owned())))
        } else {
            None
        };

        // Make the downloaded file executable.
        let mode = AddMode::new(path.to_owned()).user(Mode::Execute);
        let mut add_mode = allocator.unit(mode);
        add_mode
            .dependencies
            .extend(download.as_ref().map(|d| Dependency::Unit(d.id)));

        // Run the downloaded file.
        let mut run_once = RunOnce::new(id.to_string(), path.to_owned());
        run_once.shell = self.shell;
        run_once.root = self.root;

        for (i, arg) in self.args.iter().enumerate() {
            let arg = arg
                .as_string(facts, environment)?
                .ok_or_else(|| anyhow!("Cannot render argument #{}", i))?;

            run_once.args.push(arg);
        }

        let mut run = allocator.unit(run_once);
        run.dependencies.push(Dependency::Unit(add_mode.id));
        run.thread_local = self.interactive || self.root;

        units.extend(download);
        units.push(add_mode);
        units.push(run);

        Ok(units)
    }
}

impl fmt::Display for DownloadAndRun {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "download and run `{}`", self.url)
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
