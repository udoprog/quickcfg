//! Set up options.

use clap::{App, Arg};
use failure::Error;
use std::env;
use std::path::PathBuf;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

fn app() -> App<'static, 'static> {
    App::new("quickcfg")
        .version(VERSION)
        .author("John-John Tedro <udoprog@tedro.se>")
        .about("Configure your system, quickly!")
        .arg(
            Arg::with_name("root")
                .long("root")
                .help("Run using the given path as a configuration root.")
                .takes_value(true),
        ).arg(
            Arg::with_name("force")
                .long("force")
                .help("When updating configuration, force the update."),
        ).arg(
            Arg::with_name("debug")
                .long("debug")
                .help("Enable debug logging."),
        ).arg(
            Arg::with_name("non-interactive")
                .long("non-interactive")
                .help("Force to run in non-interactive mode."),
        ).arg(
            Arg::with_name("updates-only")
                .long("updates-only")
                .help("Only run if there are updates."),
        )
}

/// Parse command-line options.
pub fn opts() -> Result<Opts, Error> {
    let matches = app().get_matches();

    let mut opts = Opts::default();

    opts.root = matches.value_of("root").map(PathBuf::from);
    opts.force = matches.is_present("force");
    opts.non_interactive = matches.is_present("non-interactive");
    opts.updates_only = matches.is_present("updates-only");
    opts.debug = matches.is_present("debug");

    Ok(opts)
}

/// A set of parsed options.
#[derive(Default)]
pub struct Opts {
    /// The root at which the project is running from.
    pub root: Option<PathBuf>,
    /// Force update.
    pub force: bool,
    /// Run in non-interactive mode.
    pub non_interactive: bool,
    /// Only run if there are updates to the repo.
    pub updates_only: bool,
    /// Enable debug logging.
    pub debug: bool,
}

impl Opts {
    /// Find root directory based on options.
    pub fn root(&self) -> Result<PathBuf, Error> {
        match self.root.as_ref() {
            Some(root) => Ok(root.to_owned()),
            None => Ok(env::current_dir()?),
        }
    }
}
