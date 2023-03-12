use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A timestamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(SystemTime);

impl Timestamp {
    /// Construct a timestamp for now.
    pub fn now() -> Self {
        Self(SystemTime::now())
    }

    /// Get the duration since another duration.
    pub fn duration_since(self, other: Self) -> Result<Duration, std::time::SystemTimeError> {
        self.0.duration_since(other.0)
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let duration = self
            .0
            .duration_since(UNIX_EPOCH)
            .map_err(serde::ser::Error::custom)?;

        duration.as_millis().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Self(UNIX_EPOCH + Duration::from_millis(millis)))
    }
}
