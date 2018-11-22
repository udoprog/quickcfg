use directories::BaseDirs;
use failure::{format_err, Error};
use log;
use quickcfg::{
    environment as e,
    facts::Facts,
    git, hierarchy, opts, packages, stage,
    unit::{SystemUnit, Unit, UnitAllocator, UnitId, UnitInput},
    Config, DiskState, Load, Save, State, SystemInput,
};
use std::collections::HashMap;
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
    use rayon::prelude::*;

    pretty_env_logger::init();

    let opts = opts::opts()?;
    let root = opts.root()?;

    if opts.debug {
        log::set_max_level(log::LevelFilter::Trace);
    } else {
        log::set_max_level(log::LevelFilter::Info);
    }

    let state_path = root.join(".state.yml");

    let config = Config::load(&root.join("quickcfg.yml"))?.unwrap_or_default();
    let mut state = DiskState::load(&state_path)?.unwrap_or_default().to_state();

    if !update_git_and_test(&opts, &root, &mut state)? {
        return Ok(());
    }

    if opts.updates_only {
        println!("Updates found, running...");
    }

    let facts = Facts::load()?;
    let environment = e::Real;
    let data = hierarchy::load(&config.hierarchy, &root, &facts, environment)?;

    let packages = packages::Packages::detect(&facts)?;

    if let Some(packages) = packages.as_ref() {
        log::trace!("detected package manager: {}", packages.name());
    } else {
        log::warn!("no package manager detected");
    }

    let allocator = UnitAllocator::default();

    let base_dirs = BaseDirs::new();

    let input = SystemInput {
        root: &root,
        base_dirs: base_dirs.as_ref(),
        facts: &facts,
        data: &data,
        packages: packages.as_ref(),
        environment,
        allocator: &allocator,
    };

    // apply systems in parallel.
    let results = config
        .systems
        .par_iter()
        .map(|s| {
            let id = s.id();
            let requires = s.requires();
            s.apply(input).and_then(|s| Ok((id, requires, s)))
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
            system_unit.dependency(unit.id);
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
            unit.dependency(require_id);
        }

        all_units.push(unit);
    }

    // convert into stages.
    // each stage can independently be run in parallel since it's guaranteed not to have any
    // dependencies.
    let stages = stage::schedule(all_units)?;

    let input = UnitInput {
        data: &data,
        packages: packages.as_ref(),
    };

    for (i, stage) in stages.into_iter().enumerate() {
        log::trace!("stage: #{} ({} unit(s))", i, stage.units.len());

        if stage.thread_local {
            for unit in stage.units {
                unit.apply(input)?;
            }

            continue;
        }

        stage
            .units
            .into_par_iter()
            .map(|v| v.apply(input))
            .collect::<Result<_, Error>>()?;
    }

    if let Some(serialized) = state.serialize() {
        log::info!("writing dirty state: {}", state_path.display());
        serialized.save(&state_path)?;
    }

    Ok(())
}

/// Try to update git and determine if the command should keep running.
///
/// If opts.updates_only is set, we only want to continue running if we have detected changes in
/// the configuration.
fn update_git_and_test(opts: &opts::Opts, root: &Path, state: &mut State) -> Result<bool, Error> {
    if let Some(last_update) = state.last_update("git") {
        let duration = SystemTime::now().duration_since(last_update.clone())?;

        if duration.as_secs() < 10 {
            return Ok(!opts.updates_only);
        }
    };

    if !opts.non_interactive {
        if !prompt("Do you want to check for updates?")? {
            return Ok(!opts.updates_only);
        }
    }

    let git = git::Git::new(root);

    if !git.test()? {
        log::warn!("no working git command found");
        return Ok(!opts.updates_only);
    }

    if !git.needs_update()? {
        state.touch("git");
        return Ok(!opts.updates_only);
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
