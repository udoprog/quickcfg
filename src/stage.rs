//! Utilities to process a set of units into a set of inter-dependent stages.

use crate::unit::SystemUnit;
use failure::{bail, Error};
use std::collections::HashSet;

/// Discrete stage to run.
/// Unless a stage is marked as `true` in thread_local, units in this stage can be run in parallel.
pub struct Stage {
    pub thread_local: bool,
    pub units: Vec<SystemUnit>,
}

/// Schedule a collection of units into stages.
pub fn schedule(units: impl IntoIterator<Item = SystemUnit>) -> Result<Vec<Stage>, Error> {
    let mut stages = Vec::new();
    let mut units = units.into_iter().collect::<Vec<_>>();
    let mut processed = HashSet::new();

    let mut next = Vec::new();
    let mut thread_locals = Vec::new();

    while !units.is_empty() {
        // ids which have been processed in previous stages.
        let mut stage = Vec::new();

        // units that have been processed in _this_ stage.
        let mut intra = Vec::new();

        for unit in units.drain(..) {
            if !unit.dependencies().iter().all(|d| processed.contains(d)) {
                next.push(unit);
                continue;
            }

            intra.push(unit.id);

            if unit.thread_local {
                thread_locals.push(unit);
            } else {
                stage.push(unit);
            }
        }

        if stage.is_empty() && thread_locals.is_empty() {
            bail!("could not convert units to stages");
        }

        processed.extend(intra);
        stages.push(Stage {
            thread_local: false,
            units: stage,
        });

        if !thread_locals.is_empty() {
            stages.push(Stage {
                thread_local: true,
                units: thread_locals.drain(..).collect(),
            });
        }

        units.extend(next.drain(..));
    }

    Ok(stages)
}
