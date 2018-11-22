use directories::BaseDirs;
use failure::{format_err, Error, ResultExt};
use log;
use quickcfg::{
    environment as e,
    facts::Facts,
    git, hierarchy,
    opts::{self, Opts},
    packages, stage,
    unit::{SystemUnit, Unit, UnitAllocator, UnitId, UnitInput},
    Config, DiskState, FileUtils, Load, Save, State, SystemInput,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

fn main() {
    use std::process;

    if let Err(e) = try_main() {
        eprintln!("{}", e);

        for cause in e.iter_causes() {
            eprintln!("Caused by: {}", cause);
        }

        process::exit(1);
    }
}

fn try_main() -> Result<(), Error> {
    pretty_env_logger::init();

    let opts = opts::opts()?;
    let root = opts.root()?;

    if opts.debug {
        log::set_max_level(log::LevelFilter::Trace);
    } else {
        log::set_max_level(log::LevelFilter::Info);
    }

    let state_path = root.join(".state.yml");
    let state_dir = root.join(".state");

    if !state_dir.is_dir() {
        fs::create_dir(&state_dir).with_context(|_| {
            format_err!("failed to create state directory: {}", state_dir.display())
        })?;
    }

    let config = Config::load(&root.join("quickcfg.yml"))?.unwrap_or_default();
    let state = DiskState::load(&state_path)?.unwrap_or_default().to_state();

    let state = try_apply_config(&opts, &config, &root, &state_dir, state)?;

    if let Some(serialized) = state.serialize() {
        log::info!("writing dirty state: {}", state_path.display());
        serialized.save(&state_path)?;
    }

    Ok(())
}

/// Internal method to try to apply the given configuration.
fn try_apply_config(
    opts: &Opts,
    config: &Config,
    root: &Path,
    state_dir: &Path,
    mut state: State,
) -> Result<State, Error> {
    use rayon::prelude::*;

    if !try_update_config(opts, config, root, &mut state)? {
        // if we only want to run on updates, exit now.
        if opts.updates_only {
            return Ok(state);
        }
    }

    if opts.updates_only {
        log::info!("Updated found, running...");
    }

    let facts = Facts::load()?;
    let environment = e::Real;
    let data = hierarchy::load(&config.hierarchy, root, &facts, environment)?;

    let packages = packages::Packages::detect(&facts)?;

    if let Some(packages) = packages.as_ref() {
        log::trace!("detected package manager: {}", packages.name());
    } else {
        log::warn!("no package manager detected");
    }

    let allocator = UnitAllocator::default();

    let base_dirs = BaseDirs::new();
    let file_utils = FileUtils::new(state_dir, &allocator);

    // apply systems in parallel.
    let results = config
        .systems
        .par_iter()
        .map(|s| {
            let id = s.id();
            let requires = s.requires();

            let res = s.apply(SystemInput {
                root: &root,
                base_dirs: base_dirs.as_ref(),
                facts: &facts,
                data: &data,
                packages: packages.as_ref(),
                environment,
                allocator: &allocator,
                file_utils: &file_utils,
                state: &state,
            });

            res.and_then(|s| Ok((id, requires, s)))
        }).collect::<Result<Vec<_>, Error>>()?;

    let mut systems_to_units: HashMap<Option<&str>, UnitId> = HashMap::new();

    let mut all_units = Vec::new();
    let mut all_systems = Vec::new();

    for (id, requires, units) in results {
        all_systems.push((id, requires));

        let mut system_unit = allocator.unit(Unit::System);

        // allocate all IDs.
        systems_to_units.insert(id, allocator.allocate());

        for unit in &units {
            system_unit.add_dependency(unit.id);
        }

        all_units.extend(units);
    }

    for (id, requires) in all_systems {
        let unit_id = *systems_to_units
            .get(&id)
            .ok_or_else(|| format_err!("own id not present"))?;

        let mut unit = SystemUnit::new(unit_id, Unit::System);

        for require in requires {
            let require_id = *systems_to_units
                .get(&Some(require.as_str()))
                .ok_or_else(|| format_err!("could not find system with id `{}`", require))?;

            unit.add_dependency(require_id);
        }

        all_units.push(unit);
    }

    // convert into stages.
    // each stage can independently be run in parallel since it's guaranteed not to have any
    // dependencies.
    let stages = stage::schedule(all_units)?;

    for (i, stage) in stages.into_iter().enumerate() {
        log::trace!("stage: #{} ({} unit(s))", i, stage.units.len());

        if stage.thread_local {
            for unit in stage.units {
                unit.apply(UnitInput {
                    data: &data,
                    packages: packages.as_ref(),
                    state: &mut state,
                })?;
            }

            continue;
        }

        let states = stage
            .units
            .into_par_iter()
            .map(|v| {
                let mut s = State::default();

                v.apply(UnitInput {
                    data: &data,
                    packages: packages.as_ref(),
                    state: &mut s,
                })?;

                Ok(s)
            }).collect::<Result<Vec<State>, Error>>()?;

        for s in states {
            state.extend(s);
        }
    }

    Ok(state)
}

/// Try to update config from git.
///
/// Returns `true` if we have successfully downloaded a new update. `false` otherwise.
fn try_update_config(
    opts: &Opts,
    config: &Config,
    root: &Path,
    state: &mut State,
) -> Result<bool, Error> {
    if let Some(last_update) = state.last_update("git") {
        let duration = SystemTime::now().duration_since(last_update.clone())?;

        if duration < config.git_refresh {
            return Ok(false);
        }

        log::info!("{}s since last git update...", duration.as_secs());
    };

    if !opts.non_interactive {
        if !prompt("Do you want to check for updates?")? {
            return Ok(false);
        }
    }

    let git = git::Git::new(root);

    if !git.test()? {
        log::warn!("no working git command found");
        state.touch("git");
        return Ok(false);
    }

    if !git.needs_update()? {
        state.touch("git");
        return Ok(false);
    }

    if opts.force {
        git.force_update()?;
    } else {
        git.update()?;
    }

    state.touch("git");
    Ok(true)
}

/// Prompt for input.
fn prompt(question: &str) -> Result<bool, Error> {
    use std::io::{self, Write};

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();

    loop {
        write!(stdout, "{} [Y/n] ", question)?;
        stdout.flush()?;

        input.clear();
        stdin.read_line(&mut input)?;

        match input.to_lowercase().as_str().trim() {
            // NB: default.
            "" => return Ok(true),
            "y" | "ye" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {
                writeln!(stdout, "Please response with 'yes' or 'no' (or 'y' or 'n')")?;
            }
        }
    }
}
