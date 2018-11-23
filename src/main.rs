use directories::BaseDirs;
use failure::{bail, format_err, Error, ResultExt};
use log;
use quickcfg::{
    environment as e,
    facts::Facts,
    git, hierarchy,
    opts::{self, Opts},
    packages, stage,
    system::{self, SystemInput},
    unit::{self, Unit, UnitAllocator, UnitInput},
    Config, DiskState, FileUtils, Load, Save, State,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

fn report_error(e: Error) {
    let mut it = e.iter_chain();

    if let Some(e) = it.next() {
        eprintln!("{}", e);

        if let Some(bt) = e.backtrace() {
            eprintln!("{}", bt);
        }
    }

    for e in it {
        eprintln!("Caused by: {}", e);

        if let Some(bt) = e.backtrace() {
            eprintln!("{}", bt);
        }
    }
}

fn main() {
    use std::process;

    if let Err(e) = try_main() {
        report_error(e);
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

    let config_path = root.join("quickcfg.yml");
    let config = Config::load(&config_path)
        .with_context(|_| format_err!("Failed to load configuration: {}", config_path.display()))?
        .unwrap_or_default();
    let now = SystemTime::now();

    let state = DiskState::load(&state_path)?
        .unwrap_or_default()
        .to_state(&config, &now);

    let state = try_apply_config(&opts, &config, &now, &root, &state_dir, state)?;

    if let Some(serialized) = state.serialize() {
        log::trace!("Writing state: {}", state_path.display());
        serialized.save(&state_path)?;
    }

    Ok(())
}

/// Internal method to try to apply the given configuration.
fn try_apply_config<'c>(
    opts: &Opts,
    config: &'c Config,
    now: &'c SystemTime,
    root: &Path,
    state_dir: &Path,
    mut state: State<'c>,
) -> Result<State<'c>, Error> {
    use rayon::prelude::*;

    if !try_update_config(opts, config, now, root, &mut state)? {
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
    let file_utils = FileUtils::new(opts, state_dir, &allocator);

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
                now: now,
                opts: opts,
            })?;

            Ok((s, units))
        }).collect::<Result<Vec<_>, Error>>()?;

    // post-hook for all systems, mapped by id.
    let mut post_systems = HashMap::new();
    let mut all_units = Vec::new();
    let mut pre_systems = Vec::new();

    // Collect all units and map out a unit id to each system that can be used as a dependency.
    for (system, mut units) in results {
        if !system.requires().is_empty() {
            // Unit that all contained units depend on.
            // This unit finishes _before_ any unit in the system.
            let pre = allocator.unit(Unit::System);

            for unit in &mut units {
                unit.dependencies.push(unit::Dependency::Unit(pre.id));
            }

            pre_systems.push((pre, system::Dependency::Transitive(system.requires())));
        }

        if let Some(system_id) = system.id() {
            if units.is_empty() {
                // If system is empty, there is nothing to depend on.
                post_systems.insert(system_id, system::Dependency::Transitive(system.requires()));
                continue;
            }

            // Unit that other systems depend on.
            // This unit finishes _after_ all units in the system have finished.
            // System units depend on all units it contains.
            let mut post = allocator.unit(Unit::System);
            post.dependencies
                .extend(units.iter().map(|u| unit::Dependency::Unit(u.id)));
            post_systems.insert(system_id, system::Dependency::Direct(post.id));
            all_units.push(post);
        }

        all_units.extend(units);
    }

    // Wire up systems that have requires.
    for (mut pre, depend) in pre_systems {
        pre.dependencies.extend(depend.resolve(&post_systems));
        all_units.push(pre);
    }

    // Schedule all units into stages that can be run independently in parallel.
    let mut scheduler = stage::Scheduler::new(all_units);

    let mut errors = Vec::new();
    let mut i = 0;

    while let Some(stage) = scheduler.stage()? {
        i += 1;

        if log::log_enabled!(log::Level::Trace) {
            log::trace!(
                "Running stage #{} ({} unit(s)) (thread_local: {})",
                i,
                stage.units.len(),
                stage.thread_local
            );

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
                    Ok(()) => {
                        scheduler.mark(unit);
                    }
                    Err(e) => {
                        errors.push((unit, e));
                    }
                }
            }

            continue;
        }

        let results = stage
            .units
            .into_par_iter()
            .map(|unit| {
                let mut s = State::new(&config, now);

                let res = unit.apply(UnitInput {
                    data: &data,
                    packages: &packages,
                    state: &mut s,
                });

                match res {
                    Ok(()) => Ok((unit, s)),
                    Err(e) => Err((unit, e)),
                }
            }).collect::<Vec<Result<_, _>>>();

        for res in results {
            match res {
                Ok((unit, s)) => {
                    state.extend(s);
                    scheduler.mark(unit);
                }
                Err((unit, e)) => errors.push((unit, e)),
            }
        }
    }

    if !errors.is_empty() {
        for (i, (unit, e)) in errors.into_iter().enumerate() {
            log::error!("{:2}: {}", i, unit);
            report_error(e);
        }

        bail!("Failed to run all units");
    }

    let unscheduled = scheduler.into_unscheduled();

    if !unscheduled.is_empty() {
        if log::log_enabled!(log::Level::Trace) {
            log::trace!("Unable to schedule the following units:");

            for (i, unit) in unscheduled.into_iter().enumerate() {
                log::trace!("{:2}: {}", i, unit);
            }
        }

        bail!("Could not schedule all units");
    }

    Ok(state)
}

/// Try to update config from git.
///
/// Returns `true` if we have successfully downloaded a new update. `false` otherwise.
fn try_update_config(
    opts: &Opts,
    config: &Config,
    now: &SystemTime,
    root: &Path,
    state: &mut State,
) -> Result<bool, Error> {
    if let Some(last_update) = state.last_update("git") {
        let duration = now.duration_since(last_update.clone())?;

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
