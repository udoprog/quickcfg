use crate::{
    config::Config, hierarchy::Data, packages, stage::Stager, state::State, unit::{SystemUnit, UnitInput},
};
use rayon::Scope;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Scheduler, needs to be sync since it's shared across spawned tasks (and threads).
#[derive(Clone)]
pub struct Scheduler<'a, 'scope: 'a> {
    scope: &'a Scope<'scope>,
    stager: Arc<Mutex<&'a mut Stager>>,
    /// The configuration for quickcfg.
    config: &'a Config,
    now: &'a SystemTime,
    /// The prototype of unit input.
    unit_input: UnitInputPrototype<'a, 'scope>,
    /// Thread-local units to schedule.
    /// Only make sure they are running on a single thread at a time.
    thread_locals: Arc<Mutex<Vec<SystemUnit>>>,
}

impl<'a, 'scope: 'a> Scheduler<'a, 'scope> {
    /// Create a new scheduler.
    pub fn new(
        scope: &'a Scope<'scope>,
        stager: &'a mut Stager,
        config: &'a Config,
        now: &'a SystemTime,
        unit_input: UnitInputPrototype<'a, 'scope>,
    ) -> Self {
        Scheduler {
            scope,
            stager: Arc::new(Mutex::new(stager)),
            config,
            now,
            unit_input,
            thread_locals: Arc::new(Mutex::new(vec![])),
        }
    }

    /// Schedule the next batch, if possible.
    pub fn schedule_next(&self) {
        let mut stager = self.stager.lock().expect("lock poisoned");

        if let Some(stage) = stager.stage() {
            // only collect thread-locals for now, since I have no idea how to schedule them.
            if stage.thread_local {
                let mut thread_locals = self.thread_locals.lock().expect("lock poisoned");
                thread_locals.extend(stage.units);
                return;
            }

            for unit in stage.units {
                let scheduler = self.clone();
                let unit_input = self.unit_input.clone();

                self.scope.spawn(move |_| {
                    let state = State::new(scheduler.config, scheduler.now);
                    unit.apply(unit_input.to_unit_input(&mut state));
                });
            }
        }
    }

    /// Wait until terminated.
    pub fn wait(&self) {
        ()
    }
}

/// All inputs for a system.
#[derive(Clone)]
pub struct UnitInputPrototype<'a, 'scope: 'a> {
    /// Primary package manager.
    pub packages: &'a packages::Provider,
    /// Data loaded from the hierarchy.
    pub data: &'a Data,
    /// Read-only state.
    /// If none, the read state is the mutated state.
    pub read_state: &'a State<'scope>,
    /// Current timestamp.
    pub now: &'a SystemTime,
}

impl<'a, 'scope: 'a> UnitInputPrototype<'a, 'scope> {
    fn to_unit_input(self, state: &'a mut State<'scope>) -> UnitInput<'a, 'scope> {
        UnitInput {
            packages: self.packages,
            data: self.data,
            read_state: self.read_state,
            state,
            now: self.now,
        }
    }
}
