use crate::index::{Binary, HashedRange, Index, PackageFormula};
use crate::opam_version::OpamVersion;
use pubgrub::range::Range;
use pubgrub::version::Version;
use serde::Deserialize;
use std::error::Error;
use std::fs;
use walkdir::WalkDir;

/// JSON Representation of a Package File
#[derive(Debug, Deserialize)]
pub struct OpamJson {
    #[serde(rename = "opam-version")]
    pub opam_version: Option<String>,
    pub name: String,
    pub version: String,
    // Now the "depends" field is a vector of package formulas.
    pub depends: Option<Vec<OpamPackageFormula>>,
}

/// Logical operators used in both package and version formulas.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogicalOp {
    And,
    Or,
}

/// Package formulas express requirements on installed packages.
/// They can be a simple package (with optional version constraints),
/// a binary formula using a logical operator, or a group of formulas.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum OpamPackageFormula {
    /// A simple package dependency given as an object, e.g.:
    /// { "val": "B", "conditions": [ ... ] }
    Simple {
        #[serde(rename = "val")]
        name: String,
        #[serde(default)]
        conditions: Vec<OpamVersionFormula>,
    },
    /// A bare string is interpreted as a simple package with no conditions.
    Plain(String),
    /// A binary formula using a logical operator.
    /// Expects keys: "logop", "lhs", "rhs"
    Binary {
        logop: LogicalOp,
        lhs: Box<OpamPackageFormula>,
        rhs: Box<OpamPackageFormula>,
    },
    /// A grouped formula.
    /// Expects a key "group" with an array of formulas.
    Group { group: Vec<OpamPackageFormula> },
}

/// Version formulas constrain the acceptable versions for a package.
/// They support basic constraints, binary combinations, negation, and grouping.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum OpamVersionFormula {
    /// A basic version constraint, for example:
    /// { "prefix_relop": "geq", "arg": "1.0.0" }
    Constraint {
        #[serde(rename = "prefix_relop")]
        relop: String,
        arg: String,
    },
    /// A binary combination of version formulas.
    /// For example:
    /// { "logop": "and", "lhs": { ... }, "rhs": { ... } }
    Binary {
        logop: LogicalOp,
        lhs: Box<OpamVersionFormula>,
        rhs: Box<OpamVersionFormula>,
    },
    /// A negation of a version formula.
    /// For example:
    /// { "pfxop": "not", "arg": <version formula or group> }
    Not {
        #[serde(rename = "pfxop")]
        op: String, // currently only "not" is supported
        arg: Box<OpamVersionFormulaOrGroup>,
    },
    /// A grouped version formula.
    /// For example:
    /// { "group": [ { ... }, { ... } ] }
    Group { group: Vec<OpamVersionFormula> },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum OpamVersionFormulaOrGroup {
    Formula(Box<OpamVersionFormula>),
    Group { group: Vec<OpamVersionFormula> },
}

#[derive(Debug, Deserialize)]
pub struct ConditionJson {
    #[serde(rename = "prefix_relop")]
    pub relop: Option<String>,
    pub arg: Option<String>,
}

fn parse_version_formula(formula: &OpamVersionFormula) -> Range<OpamVersion> {
    match formula {
        OpamVersionFormula::Constraint { relop, arg } => {
            let val = arg.parse::<OpamVersion>().unwrap();
            let range = match relop.as_str() {
                "eq" => Range::<OpamVersion>::exact(val),
                "geq" => Range::<OpamVersion>::higher_than(val),
                "gt" => Range::<OpamVersion>::higher_than(val.bump()),
                "lt" => Range::<OpamVersion>::strictly_lower_than(val),
                "leq" => Range::<OpamVersion>::strictly_lower_than(val.bump()),
                "neq" => Range::<OpamVersion>::exact(val).negate(),
                _ => panic!("Unknown operator: {}", relop),
            };
            range
        }
        OpamVersionFormula::Binary { logop, lhs, rhs } => {
            let left = parse_version_formula(lhs);
            let right = parse_version_formula(rhs);
            match logop {
                LogicalOp::And => left.union(&right),
                LogicalOp::Or => left.intersection(&right),
            }
        }
        OpamVersionFormula::Not { op, arg } => {
            if op.to_lowercase() != "not" {
                panic!("Expected NOT operator, got: {}", op);
            }
            let inner = match **arg {
                OpamVersionFormulaOrGroup::Group { ref group } => parse_version_formula(&group[0]),
                OpamVersionFormulaOrGroup::Formula(ref boxed_formula) => {
                    parse_version_formula(boxed_formula)
                }
            };
            inner.negate()
        }
        OpamVersionFormula::Group { group } => {
            if group.is_empty() {
                panic!("Empty group");
            } else {
                parse_version_formula(&group[0])
            }
        }
    }
}

pub fn parse_package_formula(formula: &OpamPackageFormula) -> PackageFormula {
    match formula {
        OpamPackageFormula::Simple { name, conditions } => {
            let combined_range = if conditions.is_empty() {
                Range::any()
            } else {
                // Combine all conditions with AND by union-ing them together.
                conditions
                    .iter()
                    .map(|cond| parse_version_formula(cond))
                    .fold(Range::any(), |acc, r| acc.union(&r))
            };
            PackageFormula::Base {
                name: name.clone(),
                range: HashedRange(combined_range),
            }
        }
        // If it's a bare string, treat it as a simple dependency with no conditions.
        OpamPackageFormula::Plain(s) => {
            PackageFormula::Base {
                name: s.clone(),
                range: HashedRange(Range::any()),
            }
        },
        // For a binary formula, recursively convert the left- and right-hand sides.
        OpamPackageFormula::Binary { logop, lhs, rhs } => {
            let lhs_conv = parse_package_formula(lhs);
            let rhs_conv = parse_package_formula(rhs);
            let binary = Binary {
                lhs: Box::new(lhs_conv),
                rhs: Box::new(rhs_conv),
            };
            match logop {
                LogicalOp::And => PackageFormula::And(binary),
                LogicalOp::Or => PackageFormula::Or(binary),
            }
        }
        OpamPackageFormula::Group { group } => {
            if group.is_empty() {
                panic!("Empty group");
            } else {
                parse_package_formula(&group[0])
            }
        }
    }
}

/// Parse the repository by walking the directory tree.
/// For each "opam.json" file we parse the package information,
/// including its version and dependency formulas.
pub fn parse_repo(repo_path: &str) -> Result<Index, Box<dyn Error>> {
    let mut index = Index::new();
    for entry in WalkDir::new(repo_path).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() && entry.file_name() == "opam.json" {
            let content = fs::read_to_string(entry.path())?;
            let opam_data: OpamJson = serde_json::from_str(&content)?;

            let pkg_version = opam_data.version.parse::<OpamVersion>()?;

            if let Some(formulas) = opam_data.depends {
                let depends = formulas
                    .into_iter()
                    .map(|pf| parse_package_formula(&pf))
                    .collect();
                index.add_deps(&opam_data.name, pkg_version, depends);
            } else {
                index.add_deps(&opam_data.name, pkg_version, Vec::new());
            }
        }
    }
    Ok(index)
}
