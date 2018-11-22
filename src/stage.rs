//! Utilities to process a set of units into a set of inter-dependent stages.

use crate::unit::{SystemUnit, UnitId};
use failure::{bail, Error};
use std::collections::HashSet;

/// Discrete stage to run.
/// Unless a stage is marked as `true` in thread_local, units in this stage can be run in parallel.
pub struct Stage {
    pub thread_local: bool,
    pub units: Vec<SystemUnit>,
}

/// Scheduler that incrementally schedules stages to be run.
pub struct Scheduler {
    units: Vec<SystemUnit>,
    processed: HashSet<UnitId>,
    thread_locals: Vec<SystemUnit>,
    stage: Vec<SystemUnit>,
}

impl Scheduler {
    /// Construct a new scheduler out of an iterator of units.
    pub fn new(units: impl IntoIterator<Item = SystemUnit>) -> Self {
        Scheduler {
            units: units.into_iter().collect::<Vec<_>>(),
            processed: HashSet::new(),
            thread_locals: Vec::new(),
            stage: Vec::new(),
        }
    }

    /// Plans and returns the next stage to run.
    pub fn stage(&mut self) -> Result<Option<Stage>, Error> {
        let Scheduler {
            ref mut units,
            ref processed,
            ref mut thread_locals,
            ref mut stage,
        } = *self;

        loop {
            if !stage.is_empty() {
                return Ok(Some(Stage {
                    thread_local: false,
                    units: stage.drain(..).collect(),
                }));
            }

            if !thread_locals.is_empty() {
                let units = thread_locals.drain(..).collect();

                return Ok(Some(Stage {
                    thread_local: true,
                    units,
                }));
            }

            if units.is_empty() {
                return Ok(None);
            }

            // Units that roll over into the next scheduling phase.
            let mut next = Vec::new();

            for unit in units.drain(..) {
                if !unit.dependencies().iter().all(|d| processed.contains(d)) {
                    next.push(unit);
                    continue;
                }

                if unit.thread_local {
                    thread_locals.push(unit);
                } else {
                    stage.push(unit);
                }
            }

            if thread_locals.is_empty() && stage.is_empty() {
                bail!("Unable to schedule any more units");
            }

            units.extend(next);
        }
    }

    /// Mark the specified unit as successfully processed.
    pub fn mark(&mut self, unit_id: UnitId) {
        self.processed.insert(unit_id);
    }
}
