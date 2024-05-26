use anyhow::{anyhow, bail, Context as _, Error};
use directories::BaseDirs;

use quickcfg::{
    environment as e,
    facts::Facts,
    git, hierarchy,
    opts::{self, Opts},
    packages, stage,
    system::{self, SystemInput},
    unit::{self, Unit, UnitAllocator, UnitInput},
    Config, DiskState, FileSystem, Load, Save, State, Timestamp,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn report_error(e: Error) {
    let mut it = e.chain();

    if let Some(e) = it.next() {
        eprintln!("Error: {}", e);

        #[cfg(quickcfg_nightly)]
        {
            if let Some(bt) = e.backtrace() {
                eprintln!("{}", bt);
            }
        }
    }

    for e in it {
        eprintln!("Caused by: {}", e);

        #[cfg(quickcfg_nightly)]
        {
            if let Some(bt) = e.backtrace() {
                eprintln!("{}", bt);
            }
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
    pretty_env_logger::formatted_builder()
        .parse_filters("trace")
        .init();

    let base_dirs = BaseDirs::new();

    let mut opts = opts::opts()?;
    let root = opts.root(base_dirs.as_ref())?;

    let config_path = root.join("quickcfg.yml");
    let state_path = root.join(".state.yml");
    let state_dir = root.join(".state");

    if opts.paths {
        println!("OS: {}", std::env::consts::OS);
        println!("Root: {}", root.display());
        println!("Configuration File: {}", config_path.display());
        println!("State File: {}", state_path.display());
        println!("State Dir: {}", state_dir.display());
        return Ok(());
    }

    if opts.debug {
        log::set_max_level(log::LevelFilter::Trace);
    } else {
        log::set_max_level(log::LevelFilter::Info);
    }

    if !root.is_dir()
        && opts.init.is_none()
        && opts.prompt(
            "No configuration directory, would you like to set it up?",
            true,
        )?
    {
        opts.init = opts.input("[Git Repository]")?;
    }

    let git_system = git::setup().with_context(|| "failed to set up git system")?;

    if let Some(init) = opts.init.as_ref() {
        log::info!("Initializing {} from {}", root.display(), init);
        try_init(&*git_system, init, &root)?;
    } else {
        log::trace!("Using config from {}", root.display());
    }

    if !root.is_dir() {
        bail!("Missing configuration directory: {}", root.display());
    }

    if !state_dir.is_dir() {
        fs::create_dir(&state_dir).with_context(|| {
            anyhow!("Failed to create state directory: {}", state_dir.display())
        })?;
    }

    let config = Config::load(&config_path)
        .with_context(|| anyhow!("Failed to load configuration: {}", config_path.display()))?
        .unwrap_or_default();
    let now = Timestamp::now();

    let state = match DiskState::load(&state_path) {
        Ok(state) => state.unwrap_or_default(),
        Err(err) => {
            log::error!("Invalid disk state `{}`: {}", state_path.display(), err);

            if !opts.prompt("Remove it?", true)? {
                return Ok(());
            }

            DiskState::default()
        }
    };

    let mut state = state.into_state(&config, now);

    let result = try_apply_config(
        &*git_system,
        &opts,
        &config,
        now,
        base_dirs.as_ref(),
        &root,
        &state_dir,
        &mut state,
    );

    if let Some(serialized) = state.serialize() {
        log::trace!("Writing state: {}", state_path.display());
        serialized.save(&state_path)?;
    }

    result
}

/// Try to initialize the repository from the given path.
fn try_init(git_system: &dyn git::GitSystem, url: &str, root: &Path) -> Result<(), Error> {
    let _ = git::GitSystem::clone(git_system, url, root)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
/// Internal method to try to apply the given configuration.
fn try_apply_config(
    git_system: &dyn git::GitSystem,
    opts: &Opts,
    config: &Config,
    now: Timestamp,
    base_dirs: Option<&BaseDirs>,
    root: &Path,
    state_dir: &Path,
    state: &mut State<'_>,
) -> Result<(), Error> {
    use rayon::prelude::*;

    let pool = rayon::ThreadPoolBuilder::new()
        .build()
        .with_context(|| anyhow!("Failed to construct thread pool"))?;

    if !try_update_config(git_system, opts, config, now, root, state)? {
        // if we only want to run on updates, exit now.
        if opts.updates_only {
            return Ok(());
        }
    }

    if opts.updates_only {
        log::info!("Updated found, running...");
    }

    let facts = Facts::load().with_context(|| "Failed to load facts")?;
    let environment = e::Real;
    let data = hierarchy::load(&config.hierarchy, root, &facts, environment)
        .with_context(|| "Failed to load hierarchy")?;

    let packages = packages::detect(&facts)?;

    let allocator = UnitAllocator::default();

    let file_system = FileSystem::new(opts, state_dir, &allocator, &data);

    // post-hook for all systems, mapped by id.
    let mut post_systems = HashMap::new();
    let mut all_units = Vec::new();
    let mut pre_systems = Vec::new();
    let mut errors = Vec::new();

    // translate systems that needs translation.
    let systems = {
        use std::collections::VecDeque;

        let mut out = Vec::with_capacity(config.systems.len());
        let mut queue = VecDeque::new();
        queue.extend(&config.systems);

        while let Some(system) = queue.pop_back() {
            match system.translate() {
                system::Translation::Discard => {}
                system::Translation::Keep => out.push(system),
                system::Translation::Expand(systems) => queue.extend(systems),
            }
        }

        out
    };

    pool.install(|| {
        let res = systems.par_iter().map(|system| {
            let res = system.apply(SystemInput {
                root,
                base_dirs,
                facts: &facts,
                data: &data,
                packages: &packages,
                environment,
                allocator: &allocator,
                file_system: &file_system,
                state,
                now,
                opts,
                git_system,
            });

            match res {
                Ok(units) => Ok((system, units)),
                Err(e) => Err((system, e)),
            }
        });

        // Collect all units and map out a unit id to each system that can be used as a dependency.
        for res in res.collect::<Vec<_>>() {
            let (system, mut units) = match res {
                Ok(result) => result,
                Err((system, e)) => {
                    errors.push((system, e));
                    continue;
                }
            };

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
                    post_systems
                        .insert(system_id, system::Dependency::Transitive(system.requires()));
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
    });

    file_system.validate()?;

    if !errors.is_empty() {
        for (system, e) in errors.into_iter() {
            log::error!("System failed: {}", system);
            report_error(e);
        }

        bail!("Failed to run all systems");
    }

    // Wire up systems that have requires.
    for (mut pre, depend) in pre_systems {
        pre.dependencies.extend(depend.resolve(&post_systems));
        all_units.push(pre);
    }

    // Schedule all units into stages that can be run independently in parallel.
    let mut scheduler = stage::Stager::new(all_units);

    let mut errors = Vec::new();
    let mut i = 0;

    // Note: convert into a scoped pool that feeds units to be scheduled.
    pool.install(|| {
        while let Some(stage) = scheduler.stage() {
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
                    let mut s = State::new(config, now);

                    match unit.apply(UnitInput {
                        data: &data,
                        packages: &packages,
                        read_state: state,
                        state: &mut s,
                        now,
                        git_system,
                    }) {
                        Ok(()) => {
                            scheduler.mark(unit);
                        }
                        Err(e) => {
                            errors.push((unit, e));
                        }
                    }

                    state.extend(s);
                }

                continue;
            }

            let results = stage
                .units
                .into_par_iter()
                .map(|unit| {
                    let mut s = State::new(config, now);

                    let res = unit.apply(UnitInput {
                        data: &data,
                        packages: &packages,
                        read_state: state,
                        state: &mut s,
                        now,
                        git_system,
                    });

                    (res, unit, s)
                })
                .collect::<Vec<_>>();

            for (res, unit, s) in results {
                match res {
                    Ok(()) => {
                        scheduler.mark(unit);
                    }
                    Err(e) => {
                        errors.push((unit, e));
                    }
                }

                state.extend(s);
            }
        }
    });

    if !errors.is_empty() {
        for (i, (unit, e)) in errors.into_iter().enumerate() {
            log::error!("{:2}: {}", i, unit);
            report_error(e);
        }

        bail!("Failed to run all units");
    }

    let unscheduled = scheduler.into_unstaged();

    if !unscheduled.is_empty() {
        if log::log_enabled!(log::Level::Trace) {
            log::trace!("Unable to schedule the following units:");

            for (i, unit) in unscheduled.into_iter().enumerate() {
                log::trace!("{:2}: {}", i, unit);
            }
        }

        bail!("Could not schedule all units");
    }

    Ok(())
}

/// Try to update config from git.
///
/// Returns `true` if we have successfully downloaded a new update. `false` otherwise.
fn try_update_config(
    git_system: &dyn git::GitSystem,
    opts: &Opts,
    config: &Config,
    now: Timestamp,
    root: &Path,
    state: &mut State,
) -> Result<bool, Error> {
    if let Some(last_update) = state.last_update("git") {
        let duration = now.duration_since(*last_update)?;

        if duration < config.git_refresh {
            return Ok(false);
        }

        log::info!("{}s since last git update...", duration.as_secs());
    };

    if !opts.prompt("Do you want to check for updates?", true)? {
        return Ok(false);
    }

    if !git_system.test()? {
        log::warn!("no working git command found");
        state.touch("git");
        return Ok(false);
    }

    let git = git_system.open(root)?;

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
