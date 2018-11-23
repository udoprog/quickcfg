use crate::{
    environment as e,
    system::SystemInput,
    template::Template,
    unit::{AddMode, Download, RunOnce, SystemUnit},
};
use failure::{format_err, Error};
use serde_derive::Deserialize;

/// Builds one unit for every directory and file that needs to be copied.
system_struct! {
    DownloadAndRun {
        #[doc="URL to download."]
        pub url: String,
        #[doc="Run the command through /bin/sh."]
        #[serde(default)]
        pub shell: bool,
        #[doc="Arguments to add when running command."]
        #[serde(default)]
        pub args: Vec<Template>,
    }
}

impl DownloadAndRun {
    /// Copy one directory to another.
    pub fn apply<E>(&self, input: SystemInput<E>) -> Result<Vec<SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        let SystemInput {
            allocator,
            file_utils,
            state,
            facts,
            environment,
            ..
        } = input;

        let id = self
            .id
            .as_ref()
            .ok_or_else(|| format_err!("missing `id`"))?;

        if state.has_run_once(&id) {
            return Ok(vec![]);
        }

        let url = reqwest::Url::parse(&self.url)?;

        let path = file_utils.state_path(&id);

        let mut units = Vec::new();

        let download = if !path.is_file() {
            // Download the file.
            Some(allocator.unit(Download(url, path.to_owned())))
        } else {
            None
        };

        // Make the downloaded file executable.
        let mut add_mode = allocator.unit(AddMode(path.to_owned(), 0o111));
        add_mode
            .dependencies
            .extend(download.as_ref().map(|d| d.id));

        // Run the downloaded file.
        let mut run_once = RunOnce::new(id.to_string(), path.to_owned());
        run_once.shell = self.shell;

        for (i, arg) in self.args.iter().enumerate() {
            let arg = arg
                .as_string(facts, environment)?
                .ok_or_else(|| format_err!("Cannot render argument #{}", i))?;

            run_once.args.push(arg);
        }

        let mut run = allocator.unit(run_once);
        run.dependencies.push(add_mode.id);
        run.thread_local = true;

        units.extend(download);
        units.push(add_mode);
        units.push(run);

        return Ok(units);
    }
}
