use core::fmt;
use std::fmt::Display;

use pubgrub::version::Version;

// TODO https://opam.ocaml.org/doc/Manual.html#Version-ordering

/// Simplest versions possible, just a positive number.
#[derive(
    Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct OpamVersion(pub u32);

// Convert an usize into a version.
impl From<u32> for OpamVersion {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

// Convert a version into an usize.
impl From<OpamVersion> for u32 {
    fn from(version: OpamVersion) -> Self {
        version.0
    }
}

impl Display for OpamVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Version for OpamVersion {
    fn lowest() -> Self {
        Self(0)
    }
    fn bump(&self) -> Self {
        Self(self.0 + 1)
    }
}
