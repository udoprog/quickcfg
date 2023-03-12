//! Set up options.

use anyhow::{bail, Result};
use clap::Parser;
use directories::BaseDirs;
use std::path::PathBuf;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Configure your system, quickly!
#[derive(Parser)]
#[command(author = "John-John Tedro <udoprog@tedro.se>")]
pub struct Opts {
    /// Run using the given path as a configuration root.
    #[arg(long, name = "dir")]
    pub root: Option<PathBuf>,
    /// Initialize against the given repository.
    #[arg(long, name = "url")]
    pub init: Option<String>,
    /// Print paths used by quickcfg.
    #[arg(long)]
    pub paths: bool,
    /// When updating configuration, force the update.
    #[arg(long)]
    pub force: bool,
    /// Enable debug logging.
    #[arg(long)]
    pub debug: bool,
    /// Force to run in non-interactive mode.
    #[arg(long)]
    pub non_interactive: bool,
    /// Only run if there are updates.
    #[arg(long)]
    pub updates_only: bool,
}

/// Parse command-line options.
pub fn opts() -> Result<Opts> {
    let opts = Opts::try_parse()?;
    Ok(opts)
}

impl Opts {
    /// Find root directory based on options.
    pub fn root(&self, base_dirs: Option<&BaseDirs>) -> Result<PathBuf> {
        match self.root.as_ref() {
            Some(root) => Ok(root.to_owned()),
            None => match base_dirs {
                Some(base_dirs) => Ok(base_dirs.config_dir().join("quickcfg")),
                None => bail!("No base directories available"),
            },
        }
    }

    /// Prompt for yes/no.
    pub fn prompt(&self, question: &str, default: bool) -> Result<bool> {
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
    pub fn input(&self, prompt: &str) -> Result<Option<String>> {
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
