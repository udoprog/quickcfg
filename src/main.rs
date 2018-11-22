use directories::BaseDirs;
use failure::{format_err, Error, ResultExt};
use log;
use quickcfg::{
    environment as e,
    facts::Facts,
    git, hierarchy,
    opts::{self, Opts},
    packages, stage,
    unit::{Unit, UnitAllocator, UnitInput},
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
    pretty_env_logger::formatted_builder()?
        .parse("trace")
        .init();

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
            format_err!("Failed to create state directory: {}", state_dir.display())
        })?;
    }

    let config = Config::load(&root.join("quickcfg.yml"))?.unwrap_or_default();
    let state = DiskState::load(&state_path)?.unwrap_or_default().to_state();

    let state = try_apply_config(&opts, &config, &root, &state_dir, state)?;

    if let Some(serialized) = state.serialize() {
        log::info!("Writing dirty state: {}", state_path.display());
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

    let packages = packages::detect(&facts)?;

    let allocator = UnitAllocator::default();

    let base_dirs = BaseDirs::new();
    let file_utils = FileUtils::new(state_dir, &allocator);

    // apply systems in parallel.
    let results = config
        .systems
        .par_iter()
        .map(|s| {
            let units = s.apply(SystemInput {
                root: &root,
                base_dirs: base_dirs.as_ref(),
                facts: &facts,
                data: &data,
                packages: &packages,
                environment,
                allocator: &allocator,
                file_utils: &file_utils,
                state: &state,
            })?;

            Ok((s, units))
        }).collect::<Result<Vec<_>, Error>>()?;

    // post-hook for all systems, mapped by id.
    let mut post_systems = HashMap::new();
    let mut all_units = Vec::new();
    let mut systems_with_requires = Vec::new();

    // Collect all units and map out a unit id to each system that can be used as a dependency.
    for (system, mut units) in results {
        if !system.requires().is_empty() {
            // unit that all contained units depend on.
            // this is also the unit which we wire up system dependencies from.
            let pre = allocator.unit(Unit::System);

            for unit in &mut units {
                unit.add_dependency(pre.id);
            }

            systems_with_requires.push((system, pre));
        }

        if let Some(system_id) = system.id() {
            // unit that other system can depend on.
            // this is the unit that other systems depend on.
            let mut post = allocator.unit(Unit::System);

            // system units depend on all units it contains.
            post.add_dependencies(units.iter().map(|u| u.id));

            // allocate all IDs.
            post_systems.insert(system_id, post.id);

            all_units.push(post);
        }

        all_units.extend(units);
    }

    // Wire up systems that have requires.
    for (system, mut pre) in systems_with_requires {
        for require in system.requires() {
            let require_id = *post_systems
                .get(require.as_str())
                .ok_or_else(|| format_err!("Could not find system with id `{}`", require))?;

            pre.add_dependency(require_id);
        }

        all_units.push(pre);
    }

    // Schedule all units into stages that can be run independently in parallel.
    let mut scheduler = stage::Scheduler::new(all_units);

    let mut errors = Vec::new();
    let mut i = 0;

    while let Some(stage) = scheduler.stage()? {
        i += 1;

        if log::log_enabled!(log::Level::Trace) {
            log::trace!("Running stage #{} ({} unit(s))", i, stage.units.len());

            for (i, unit) in stage.units.iter().enumerate() {
                log::trace!("{:2}: {}", i, unit);
            }
        }

        if stage.thread_local {
            for unit in stage.units {
                match unit.apply(UnitInput {
                    data: &data,
                    packages: &packages,
                    state: &mut state,
                }) {
                    Err(e) => errors.push(e),
                    Ok(()) => scheduler.mark(unit.id),
                }
            }

            continue;
        }

        let results = stage
            .units
            .into_par_iter()
            .map(|unit| {
                let mut s = State::default();

                unit.apply(UnitInput {
                    data: &data,
                    packages: &packages,
                    state: &mut s,
                })?;

                Ok((unit.id, s))
            }).collect::<Vec<Result<_, Error>>>();

        for res in results {
            match res {
                Ok((id, s)) => {
                    state.extend(s);
                    scheduler.mark(id);
                }
                Err(e) => errors.push(e),
            }
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
