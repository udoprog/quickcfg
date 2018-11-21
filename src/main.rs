use failure::{bail, format_err, Error};
use log::trace;
use quickcfg::{
    environment as e, Config, SystemInput, SystemUnit, Unit, UnitAllocator, UnitId,
    hierarchy,
    facts,
    packages,
    UnitInput,
};
use serde_yaml;
use std::collections::HashMap;
use std::env;
use std::error;
use std::fs::File;
use std::path::Path;
use directories::BaseDirs;

fn main() -> Result<(), Box<error::Error>> {
    pretty_env_logger::init();

    use rayon::prelude::*;

    let root = env::current_dir()?;
    let config = root.join("config.yml");

    let config: quickcfg::Config = if config.is_file() {
        load_config(&config)?
    } else {
        Default::default()
    };

    let facts = facts::load()?;
    let environment = e::Real;
    let data = hierarchy::load(&config.hierarchy, &root, &facts, environment)?;

    let packages = packages::Packages::detect()?;

    trace!("Detected package manager: {:?}", packages);

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
            system_unit.dependency(unit.id());
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
    let stages = convert_to_stages(all_units)?;

    let input = UnitInput { data: &data, packages: packages.as_ref(), };

    for (i, stage) in stages.into_iter().enumerate() {
        trace!("stage: #{} ({} unit(s))", i, stage.units.len());

        stage
            .units
            .into_par_iter()
            .map(|v| v.apply(input))
            .collect::<Result<_, Error>>()?;
    }

    Ok(())
}

/// Discrete stages to run.
struct Stage {
    units: Vec<SystemUnit>,
}

/// Convert all units into stages.
fn convert_to_stages(units: impl IntoIterator<Item = SystemUnit>) -> Result<Vec<Stage>, Error> {
    use std::collections::HashSet;

    let mut stages = Vec::new();
    let mut units = units.into_iter().collect::<Vec<_>>();
    let mut processed = HashSet::new();

    while !units.is_empty() {
        // ids which have been processed in previous stages.
        let mut stage = Vec::new();
        // units which have been processed in _this_ stage.
        let mut intra = Vec::new();

        for unit in units.drain(..).collect::<Vec<_>>() {
            if unit.dependencies().iter().all(|d| processed.contains(d)) {
                intra.push(unit.id());
                stage.push(unit);
            } else {
                units.push(unit);
            }
        }

        if stage.is_empty() {
            bail!("could not convert units to stages");
        }

        processed.extend(intra);
        stages.push(Stage { units: stage });
    }

    Ok(stages)
}

/// Load configuration from the given path.
fn load_config(path: &Path) -> Result<Config, Error> {
    let f = File::open(path)
        .map_err(|e| format_err!("failed to open config: {}: {}", path.display(), e))?;
    let c = serde_yaml::from_reader(f)
        .map_err(|e| format_err!("failed to parse config: {}: {}", path.display(), e))?;
    Ok(c)
}
