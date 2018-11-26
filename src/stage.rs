//! Utilities to process a set of units into a set of inter-dependent stages.

use crate::unit::{Dependency, SystemUnit};
use std::collections::HashSet;

/// Discrete stage to run.
/// Unless a stage is marked as `true` in thread_local, units in this stage can be run in parallel.
pub struct Stage {
    pub thread_local: bool,
    pub units: Vec<SystemUnit>,
}

/// Stager that incrementally schedules stages to be run.
pub struct Stager {
    units: Vec<SystemUnit>,
    provided: HashSet<Dependency>,
    thread_locals: Vec<SystemUnit>,
    stage: Vec<SystemUnit>,
}

impl Stager {
    /// Construct a new stager out of an iterator of units.
    pub fn new(units: impl IntoIterator<Item = SystemUnit>) -> Self {
        Stager {
            units: units.into_iter().collect::<Vec<_>>(),
            provided: HashSet::new(),
            thread_locals: Vec::new(),
            stage: Vec::new(),
        }
    }

    /// Plans and returns the next stage to run.
    pub fn stage(&mut self) -> Option<Stage> {
        let Stager {
            ref mut units,
            ref provided,
            ref mut thread_locals,
            ref mut stage,
        } = *self;

        loop {
            // Units that roll over into the next scheduling phase.
            let mut next = Vec::new();

            for unit in units.drain(..) {
                if !unit.dependencies.iter().all(|d| provided.contains(d)) {
                    next.push(unit);
                    continue;
                }

                if unit.thread_local {
                    thread_locals.push(unit);
                } else {
                    stage.push(unit);
                }
            }

            units.extend(next);

            if !stage.is_empty() {
                return Some(Stage {
                    thread_local: false,
                    units: stage.drain(..).collect(),
                });
            }

            if !thread_locals.is_empty() {
                let units = thread_locals.drain(..).collect();

                return Some(Stage {
                    thread_local: true,
                    units,
                });
            }

            if thread_locals.is_empty() && stage.is_empty() {
                return None;
            }
        }
    }

    /// Mark the specified unit as successfully processed.
    pub fn mark(&mut self, unit: SystemUnit) {
        log::trace!("Mark: {}", unit);
        self.provided.extend(unit.provides.into_iter());
        self.provided.insert(Dependency::Unit(unit.id));
    }

    /// Convert into unscheduled units.
    pub fn into_unstaged(self) -> Vec<SystemUnit> {
        self.units
    }
}
