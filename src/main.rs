use failure::{bail, format_err, Error};
use log::{info, trace};
use quickcfg::{
    environment as e, Config, SystemInput, SystemUnit, Template, UnitAllocator, UnitInput,
};
use serde_yaml;
use std::collections::HashMap;
use std::env;
use std::error;
use std::fs::{self, File};
use std::io;
use std::path::Path;

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

    let facts = load_facts()?;
    let environment = e::Real;
    let data = load_hierarchy(&config.hierarchy, &root, &facts, environment)?;

    let allocator = UnitAllocator::default();

    let input = SystemInput {
        root: &root,
        facts: &facts,
        environment,
        allocator: &allocator,
    };

    let mut systems_to_units: HashMap<Option<String>, Vec<SystemUnit>> = HashMap::new();

    for system in config.systems {
        let id = system.id();
        let units = system.apply(input)?;
        systems_to_units.entry(id).or_default().extend(units);
    }

    // convert into stages.
    // each stage can independently be run in parallel since it's guaranteed not to have any
    // dependencies.
    let stages = convert_to_stages(
        systems_to_units
            .into_iter()
            .flat_map(|(_, units)| units.into_iter()),
    )?;

    let input = UnitInput { data: &data };

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

/// Load a hierarchy.
fn load_hierarchy<'a>(
    it: impl IntoIterator<Item = &'a Template>,
    root: &Path,
    facts: &HashMap<String, String>,
    environment: impl Copy + e::Environment,
) -> Result<serde_yaml::Value, Error> {
    use serde_yaml::{Mapping, Value};

    let mut map = Mapping::new();

    for h in it {
        let path = match h.render_as_relative_path(facts, environment)? {
            None => continue,
            Some(path) => path,
        };

        let path = path.to_path(root);

        extend_from_hierarchy(&mut map, &path).map_err(|e| {
            format_err!("failed to extend hierarchy from: {}: {}", path.display(), e)
        })?;
    }

    return Ok(Value::Mapping(map));

    /// Extend the existing mapping from the given hierarchy.
    fn extend_from_hierarchy(map: &mut Mapping, path: &Path) -> Result<(), Error> {
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => return Ok(()),
                _ => bail!("failed to open file: {}", e),
            },
        };

        match serde_yaml::from_reader(file)? {
            Value::Mapping(m) => {
                map.extend(m);
            }
            other => {
                bail!(
                    "Cannot deal with value `{:?}` from hierarchy: {}",
                    other,
                    path.display()
                );
            }
        }

        info!("LOAD PATH: {}", path.display());
        Ok(())
    }
}

/// Load facts.
fn load_facts() -> Result<HashMap<String, String>, Error> {
    let mut facts = HashMap::new();

    if let Some(distro) = detect_distro()? {
        facts.insert("distro".to_string(), distro);
    }

    return Ok(facts);

    /// Detect which distro we appear to be running.
    fn detect_distro() -> Result<Option<String>, Error> {
        if metadata("/etc/redhat-release")?
            .map(|m| m.is_file())
            .unwrap_or(false)
        {
            return Ok(Some("fedora".to_string()));
        }

        if metadata("/etc/gentoo-release")?
            .map(|m| m.is_file())
            .unwrap_or(false)
        {
            return Ok(Some("gentoo".to_string()));
        }

        if metadata("/etc/debian_version")?
            .map(|m| m.is_file())
            .unwrap_or(false)
        {
            return Ok(Some("debian".to_string()));
        }

        if environ("OSTYPE")?
            .map(|s| s.starts_with("darwin"))
            .unwrap_or(false)
        {
            return Ok(Some("osx".to_string()));
        }

        Ok(None)
    }

    fn metadata<P: AsRef<Path>>(path: P) -> Result<Option<fs::Metadata>, Error> {
        let p = path.as_ref();

        let m = match fs::metadata(p) {
            Ok(m) => m,
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => return Ok(None),
                _ => bail!("failed to load file metadata: {}: {}", p.display(), e),
            },
        };

        Ok(Some(m))
    }

    fn environ(key: &str) -> Result<Option<String>, Error> {
        let value = match env::var(key) {
            Ok(value) => value,
            Err(env::VarError::NotPresent) => return Ok(None),
            Err(e) => bail!("failed to load environment var: {}: {}", key, e),
        };

        Ok(Some(value))
    }
}

/// Load configuration from the given path.
fn load_config(path: &Path) -> Result<Config, Error> {
    let f = File::open(path)
        .map_err(|e| format_err!("failed to open config: {}: {}", path.display(), e))?;
    let c = serde_yaml::from_reader(f)
        .map_err(|e| format_err!("failed to parse config: {}: {}", path.display(), e))?;
    Ok(c)
}
