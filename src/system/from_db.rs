use crate::{
    environment as e,
    system::{System, SystemInput, SystemUnit, Translation},
    unit,
};
use anyhow::Result;
use std::fmt;

system_struct! {
    #[doc = "Read a system from the database."]
    FromDb {
        #[doc="The type of the system to instantiate."]
        pub system: String,
        #[doc="The key to use when instantiating from the database."]
        pub key: String,
    }
}

impl FromDb {
    system_defaults!(translate);

    /// Copy one directory to another.
    pub fn apply<E>(&self, input: SystemInput<E>) -> Result<Vec<SystemUnit>>
    where
        E: Copy + e::Environment,
    {
        use serde_yaml::Value;

        let SystemInput {
            allocator, data, ..
        } = input;

        let mut unit = allocator.unit(unit::FromDb {
            system: self.system.clone(),
            key: self.key.clone(),
        });

        let systems = data.load_array::<serde_yaml::Mapping>(&self.system)?;
        let mut out = Vec::new();

        for mut system in systems {
            system.insert("type".into(), self.system.clone().into());
            let system = serde_yaml::from_value::<System>(Value::Mapping(system))?;

            match system.translate() {
                Translation::Discard => continue,
                Translation::Keep => {
                    for s in system.apply(input)? {
                        unit.dependencies.push(unit::Dependency::Unit(s.id));
                        out.push(s);
                    }
                }
                Translation::Expand(systems) => {
                    for system in systems {
                        for s in system.apply(input)? {
                            unit.dependencies.push(unit::Dependency::Unit(s.id));
                            out.push(s);
                        }
                    }

                    continue;
                }
            }
        }

        out.push(unit);
        Ok(out)
    }
}

impl fmt::Display for FromDb {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(
            fmt,
            "system `{}` from database key `{}`",
            self.system, self.key
        )
    }
}
