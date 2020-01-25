use crate::{
    environment as e,
    system::{System, SystemInput, SystemUnit, Translation},
};
use anyhow::{bail, Error};
use std::fmt;

system_struct! {
    #[doc = "Conditionally run only for the given operating system."]
    OnlyFor {
        #[doc="Which OS to run the given systems for."]
        pub os: Option<String>,
        pub systems: Vec<System>,
    }
}

impl OnlyFor {
    pub fn translate(&self) -> Translation<'_> {
        if let Some(os) = self.os.as_ref() {
            match (os.as_str(), std::env::consts::OS) {
                (current, actual) if current == actual => (),
                ("unix", "linux") => (),
                ("unix", "macos") => (),
                _ => return Translation::Discard,
            }
        }

        Translation::Expand(&self.systems)
    }

    /// Copy one directory to another.
    pub fn apply<E>(&self, _: SystemInput<E>) -> Result<Vec<SystemUnit>, Error>
    where
        E: Copy + e::Environment,
    {
        bail!("Cannot apply only-for systems");
    }
}

impl fmt::Display for OnlyFor {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "conditionally run for (os: {:?})", self.os)
    }
}
