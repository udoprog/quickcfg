//! Things to do.

use crate::{
    environment as e,
    unit::{SystemUnit, UnitAllocator},
};
use failure::Error;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::path::Path;

mod copy_dir;

use self::copy_dir::CopyDir;

type Facts = HashMap<String, String>;

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum System {
    #[serde(rename = "copy-dir")]
    CopyDir(CopyDir),
}

impl System {
    system_functions!(CopyDir);
}

/// All inputs for a system.
#[derive(Clone, Copy)]
pub struct SystemInput<'a, E>
where
    E: Copy + e::Environment,
{
    /// The root directory where all relative paths are referenced from.
    pub root: &'a Path,
    /// Set of facts.
    pub facts: &'a Facts,
    /// Source of environment variables.
    pub environment: E,
    /// Unit allocator to use.
    pub allocator: &'a UnitAllocator,
}
