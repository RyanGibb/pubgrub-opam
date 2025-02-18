use pubgrub::range::Range;
use core::fmt::Display;
use std::cell::Cell;
use std::hash::{Hash, Hasher};

use crate::opam_version::OpamVersion;
use crate::parse::available_versions_from_repo;

pub type PackageName = String;

pub struct Index {
    pub repo: String,
    pub debug: Cell<bool>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Binary<T> {
    pub lhs: Box<T>,
    pub rhs: Box<T>,
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
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum VersionFormula {
    Version(HashedRange),
    Lit(OpamVersion),
    Variable(String),
    Not(String),
    Eq(Binary<VersionFormula>),
    Neq(Binary<VersionFormula>),
    Geq(Binary<VersionFormula>),
    Gt(Binary<VersionFormula>),
    Leq(Binary<VersionFormula>),
    Lt(Binary<VersionFormula>),
    Or(Binary<VersionFormula>),
    And(Binary<VersionFormula>),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum PackageFormula {
    Or(Binary<PackageFormula>),
    And(Binary<PackageFormula>),
    Base {
        name: PackageName,
        formula: VersionFormula,
    },
}

impl Display for VersionFormula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionFormula::Variable(variable) => {
                write!(f, "{}", variable)
            }
            VersionFormula::Lit(literal) => {
                write!(f, "{}", literal)
            }
            VersionFormula::Version(version) => {
                write!(f, "= {}", version)
            }
            VersionFormula::Eq(binary) => {
                write!(f, "({} = {})", binary.lhs, binary.rhs)
            }
            VersionFormula::Geq(binary) => {
                write!(f, "({} >= {})", binary.lhs, binary.rhs)
            }
            VersionFormula::Gt(binary) => {
                write!(f, "({} > {})", binary.lhs, binary.rhs)
            }
            VersionFormula::Leq(binary) => {
                write!(f, "({} <= {})", binary.lhs, binary.rhs)
            }
            VersionFormula::Lt(binary) => {
                write!(f, "({} < {})", binary.lhs, binary.rhs)
            }
            VersionFormula::Neq(binary) => {
                write!(f, "({} != {})", binary.lhs, binary.rhs)
            }
            VersionFormula::And(binary) => {
                write!(f, "({} & {})", binary.lhs, binary.rhs)
            }
            VersionFormula::Or(binary) => {
                write!(f, "({} | {})", binary.lhs, binary.rhs)
            }
            VersionFormula::Not(variable) => {
                write!(f, "!{}", variable)
            }
        }
    }
}

impl Display for PackageFormula {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageFormula::Base { name, formula } => {
                write!(f, "({} {{{}}})", name, formula)
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
    pub fn new(repo: String) -> Self {
        Self {
            repo,
            debug : false.into()
        }
    }

    pub fn available_versions(&self, package: &PackageName) -> Vec<OpamVersion> {
        available_versions_from_repo(self.repo.as_str(), package).unwrap()
    }

    pub fn set_debug(&self, flag: bool) {
        self.debug.set(flag);
    }
}
