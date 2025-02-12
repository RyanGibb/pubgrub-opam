use crate::index::{Binary, HashedRange, PackageFormula};
use crate::opam_version::OpamVersion;
use pubgrub::range::Range;
use pubgrub::version::Version;
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::str::FromStr;

/// JSON Representation of a Package File
#[derive(Debug, Deserialize)]
pub struct OpamJson {
    #[serde(rename = "opam-version")]
    pub opam_version: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
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
    /// A bare string is interpreted as a simple package with no conditions.
    Plain(String),
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
        arg: FilterExpr,
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
        arg: Box<OpamVersionFormula>,
    },
    /// A grouped version formula.
    /// For example:
    /// { "group": [ { ... }, { ... } ] }
    Group { group: Vec<OpamVersionFormula> },
    Filter(FilterExpr),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum FilterExpr {
    /// An object of the form { "id": "version" } represents a variable.
    Var { id: String },
    /// A literal value (we assume it’s a string literal).
    Lit(String),
}

fn parse_version_formula(formula: &OpamVersionFormula) -> Range<OpamVersion> {
    match formula {
        OpamVersionFormula::Constraint { relop, arg } => {
            // TODO parse filter
            let val = match arg {
                FilterExpr::Var { id } => id,
                FilterExpr::Lit(lit) => lit,
            };
            let version = val.parse::<OpamVersion>().unwrap();
            let range = match relop.as_str() {
                "eq" => Range::<OpamVersion>::exact(version),
                "geq" => Range::<OpamVersion>::higher_than(version),
                "gt" => Range::<OpamVersion>::higher_than(version.bump()),
                "lt" => Range::<OpamVersion>::strictly_lower_than(version),
                "leq" => Range::<OpamVersion>::strictly_lower_than(version.bump()),
                "neq" => Range::<OpamVersion>::exact(version).negate(),
                _ => panic!("Unknown operator: {}", relop),
            };
            range
        }
        OpamVersionFormula::Binary { logop, lhs, rhs } => {
            let left = parse_version_formula(lhs);
            let right = parse_version_formula(rhs);
            match logop {
                LogicalOp::And => left.intersection(&right),
                LogicalOp::Or => left.union(&right),
            }
        }
        OpamVersionFormula::Not { op, arg } => {
            match op.as_str() {
                "not" => {
                    let inner = parse_version_formula(*&arg);
                    inner.negate()
                },
                // TODO
                "defined" => {
                    Range::any()
                }
                op => panic!("Unrecognised NOT operator {}", op)
            }
        }
        OpamVersionFormula::Group { group } => {
            if group.is_empty() {
                panic!("Empty group");
            } else {
                parse_version_formula(&group[0])
            }
        }
        OpamVersionFormula::Filter(_filter_expr) => {
            Range::any()
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
                    .fold(Range::none(), |acc, r| acc.union(&r))
            };
            PackageFormula::Base {
                name: name.clone(),
                range: HashedRange(combined_range),
            }
        }
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
        OpamPackageFormula::Plain(s) => {
            PackageFormula::Base {
                name: s.clone(),
                range: HashedRange(Range::any()),
            }
        }

    }
}

/// Given a repository path and a package name, returns a vector of available versions
/// for that package, in descending order (newest first).
///
/// The repository is assumed to have the following structure:
///   repo_path/package-name/package-name.version/opam.json
pub fn available_versions_from_repo(repo_path: &str, package: &str) -> Result<Vec<OpamVersion>, Box<dyn Error>> {
    // Construct the package directory: repo_path/package
    let pkg_dir = Path::new(repo_path).join(package);
    if !pkg_dir.exists() {
        return Err(format!("Package path {} does not exist", pkg_dir.display()).into());
    }

    let mut versions = Vec::new();
    // Read the package directory: each subdirectory is assumed to be a version folder.
    for entry in fs::read_dir(&pkg_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            // Get the directory name (e.g. "A.2.0.0")
            let dir_name = entry.file_name();
            let dir_str = dir_name.to_string_lossy();
            // Assume the directory name starts with "package." and then the version.
            let prefix = format!("{}.", package);
            let ver_str = if dir_str.starts_with(&prefix) {
                // Strip the package prefix and the dot.
                &dir_str[prefix.len()..]
            } else {
                // Fallback: try using the entire directory name.
                &dir_str
            };
            // Parse the version string into an OpamVersion.
            let version = OpamVersion::from_str(ver_str)?;
            versions.push(version);
        }
    }
    // Sort the versions in ascending order and then reverse for descending order.
    versions.sort();
    versions.reverse();
    Ok(versions)
}

/// Given a repository path, package name, and version,
/// returns the dependency formulas for that package version.
pub fn parse_dependencies_for_package_version(
    repo_path: &str,
    package: &str,
    version: &str,
) -> Result<Vec<PackageFormula>, Box<dyn Error>> {
    // Build the expected directory path.
    // For example:
    //   repo_path/packages/A/A.2.0.0/opam.json
    let pkg_dir = Path::new(repo_path)
        .join(package)
        .join(format!("{}.{}", package, version));
    let opam_file = pkg_dir.join("opam.json");

    // Read the opam file.
    let content = fs::read_to_string(&opam_file)
        .map_err(|e| format!("Failed to read {}: {}", opam_file.display(), e))?;

    // Parse the JSON into an OpamJson struct.
    let opam_data: OpamJson = serde_json::from_str(&content)
        .map_err(|e| format!("Error parsing {}: {}\nContent:\n{}", opam_file.display(), e, content))?;

    // Convert the dependency formulas, if any.
    if let Some(formulas) = opam_data.depends {
        let dependencies = formulas
            .into_iter()
            .map(|pf| parse_package_formula(&pf))
            .collect();
        Ok(dependencies)
    } else {
        Ok(Vec::new())
    }
}
