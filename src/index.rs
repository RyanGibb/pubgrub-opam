use pubgrub::range::Range;
use core::fmt::Display;
use std::hash::{Hash, Hasher};

use crate::opam_version::OpamVersion;
use crate::parse::available_versions_from_repo;

pub type PackageName = String;

pub struct Index {
    pub repo: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Binary {
    pub lhs: Box<PackageFormula>,
    pub rhs: Box<PackageFormula>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HashedRange(pub Range<OpamVersion>);

impl Hash for HashedRange {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let s = format!("{}", self.0);
        s.hash(state);
    }
}

impl Display for HashedRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Delegate to the Display implementation of the inner Range.
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum PackageFormula {
    Or(Binary),
    And(Binary),
    Base {
        name: PackageName,
        range: HashedRange,
    },
}

impl Display for PackageFormula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageFormula::Base { name, range } => {
                write!(f, "({}: {})", name, range)
            }
            PackageFormula::And(binary) => {
                write!(f, "({} & {})", binary.lhs, binary.rhs)
            }
            PackageFormula::Or(binary) => {
                write!(f, "({} | {})", binary.lhs, binary.rhs)
            }
        }
    }
}

impl Index {
    /// Empty new index.
    pub fn new(repo: String) -> Self {
        Self {
            repo,
        }
    }

    /// List existing versions for a given package with newest versions first.
    pub fn available_versions(&self, package: &PackageName) -> Vec<OpamVersion> {
        available_versions_from_repo(self.repo.as_str(), package).unwrap()
    }
}
