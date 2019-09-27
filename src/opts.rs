//! Set up options.

use clap::{App, Arg};
use directories::BaseDirs;
use failure::{bail, Error};
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
        )
        .arg(
            Arg::with_name("init")
                .long("init")
                .help("Initialize against the given repository.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("paths")
                .long("paths")
                .help("Print paths used by quickcfg."),
        )
        .arg(
            Arg::with_name("force")
                .long("force")
                .help("When updating configuration, force the update."),
        )
        .arg(
            Arg::with_name("debug")
                .long("debug")
                .help("Enable debug logging."),
        )
        .arg(
            Arg::with_name("non-interactive")
                .long("non-interactive")
                .help("Force to run in non-interactive mode."),
        )
        .arg(
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
    opts.init = matches.value_of("init").map(String::from);
    opts.paths = matches.is_present("paths");
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
    /// Initialize the project from the given repo.
    pub init: Option<String>,
    /// Print paths used by quickcfg.
    pub paths: bool,
    /// Force update.
    pub force: bool,
    /// Run in non-interactive mode.
    non_interactive: bool,
    /// Only run if there are updates to the repo.
    pub updates_only: bool,
    /// Enable debug logging.
    pub debug: bool,
}

impl Opts {
    /// Find root directory based on options.
    pub fn root(&self, base_dirs: Option<&BaseDirs>) -> Result<PathBuf, Error> {
        match self.root.as_ref() {
            Some(root) => Ok(root.to_owned()),
            None => match base_dirs {
                Some(base_dirs) => Ok(base_dirs.config_dir().join("quickcfg")),
                None => bail!("No base directories available"),
            },
        }
    }

    /// Prompt for yes/no.
    pub fn prompt(&self, question: &str, default: bool) -> Result<bool, Error> {
        use std::io::{self, Write};

        if self.non_interactive {
            return Ok(default);
        }

        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let mut input = String::new();

        let p = if default { "[Y/n]" } else { "[y/N]" };

        loop {
            write!(stdout, "{} {} ", question, p)?;
            stdout.flush()?;

            input.clear();
            stdin.read_line(&mut input)?;

            match input.to_lowercase().as_str().trim() {
                // NB: default.
                "" => return Ok(default),
                "y" | "ye" | "yes" => return Ok(true),
                "n" | "no" => return Ok(false),
                _ => {
                    writeln!(stdout, "Please response with 'yes' or 'no' (or 'y' or 'n')")?;
                }
            }
        }
    }

    /// Prompt for input.
    pub fn input(&self, prompt: &str) -> Result<Option<String>, Error> {
        use std::io::{self, Write};

        if self.non_interactive {
            return Ok(None);
        }

        let stdin = io::stdin();
        let mut stdout = io::stdout();

        write!(stdout, "{} ", prompt)?;
        stdout.flush()?;

        let mut input = String::new();
        stdin.read_line(&mut input)?;

        Ok(Some(input.trim().to_string()))
    }
}
