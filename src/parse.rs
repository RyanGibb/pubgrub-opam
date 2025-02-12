use crate::index::{Binary, HashedRange, PackageFormula};
use crate::opam_version::OpamVersion;
use pubgrub::range::Range;
use pubgrub::version::Version;
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::str::FromStr;

#[derive(Debug, Deserialize)]
pub struct OpamJson {
    #[serde(rename = "opam-version")]
    pub opam_version: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub depends: Option<DependsField>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DependsField {
    Single(OpamPackageFormula),
    Multiple(Vec<OpamPackageFormula>),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum OpamPackageFormula {
    Binary {
        logop: LogicalOp,
        lhs: Box<OpamPackageFormula>,
        rhs: Box<OpamPackageFormula>,
    },
    Group { group: Vec<OpamPackageFormula> },
    Simple {
        #[serde(rename = "val")]
        name: String,
        conditions: Vec<OpamVersionFormula>,
    },
    Plain(String),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnaryOp {
    Not,
    Defined,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RelOp {
    Eq,
    Geq,
    Gt,
    Leq,
    Lt,
    Neq
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum OpamVersionFormula {
    LogOp {
        logop: LogicalOp,
        lhs: Box<OpamVersionFormula>,
        rhs: Box<OpamVersionFormula>,
    },
    Group { group: Vec<OpamVersionFormula> },
    PrefixOperator {
        pfxop: UnaryOp,
        arg: Box<OpamVersionFormula>,
    },
    PrefixRelop {
        prefix_relop: RelOp,
        arg: FilterOrVersion,
    },
    Filter(FilterExpr),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum FilterOrVersion {
    Version(String),
    Filter(FilterExpr)
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum LiteralValue {
    Str(String),
    Int(i64),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum FilterExpr {
    LogOp {
        logop: LogicalOp,
        lhs: Box<FilterExpr>,
        rhs: Box<FilterExpr>,
    },
    Unary {
        pfxop: String,
        arg: Box<FilterExpr>,
    },
    Group { group: Vec<FilterExpr> },
    Relop {
        relop: RelOp,
        lhs: Box<FilterExpr>,
        rhs: Box<FilterExpr>,
    },
    Variable { id: String },
    Literal(LiteralValue),
}

fn parse_version_formula(formula: &OpamVersionFormula) -> Option<Range<OpamVersion>> {
    match formula {
        OpamVersionFormula::LogOp { logop, lhs, rhs } => {
            let left = parse_version_formula(lhs);
            let right = parse_version_formula(rhs);
            match logop {
                LogicalOp::And =>
                    match (left, right) {
                        (Some(l), Some(r)) => Some(l.intersection(&r)),
                        _ => None
                    },
                LogicalOp::Or =>
                    match (left, right) {
                        (Some(l), Some(r)) => Some(l.union(&r)),
                        (Some(l), None) => Some(l),
                        (None, Some(r)) => Some(r),
                        (None, None) => None,
                    },
            }
        }
        OpamVersionFormula::PrefixRelop { prefix_relop, arg } => {
            match arg {
                FilterOrVersion::Version(version) => {
                    let version = version.parse::<OpamVersion>().unwrap();
                    let range = match prefix_relop {
                        RelOp::Eq => Range::<OpamVersion>::exact(version),
                        RelOp::Geq => Range::<OpamVersion>::higher_than(version),
                        RelOp::Gt => Range::<OpamVersion>::higher_than(version.bump()),
                        RelOp::Lt => Range::<OpamVersion>::strictly_lower_than(version),
                        RelOp::Leq => Range::<OpamVersion>::strictly_lower_than(version.bump()),
                        RelOp::Neq => Range::<OpamVersion>::exact(version).negate(),
                    };
                    Some(range)
                },
                // TODO parse filter
                _ => None
            }
        }
        OpamVersionFormula::Group { group } => {
            if group.is_empty() {
                panic!("Empty group");
            } else {
                parse_version_formula(&group[0])
            }
        }
        OpamVersionFormula::PrefixOperator { pfxop, arg } => {
            match pfxop {
                UnaryOp::Not => {
                    let inner = parse_version_formula(*&arg)?;
                    Some(inner.negate())
                },
                UnaryOp::Defined => {
                    // TODO
                    None
                }
            }
        }
        OpamVersionFormula::Filter(_filter_expr) => {
            None
        }
    }
}

pub fn parse_package_formula(formula: &OpamPackageFormula) -> Option<PackageFormula> {
    match formula {
        OpamPackageFormula::Simple { name, conditions } => {
            let combined_range = if conditions.is_empty() {
                Some(Range::any())
            } else {
                parse_version_formula(&conditions[0])
            }?;
            let base = PackageFormula::Base {
                name: name.clone(),
                range: HashedRange(combined_range),
            };
            Some(base)
        }
        // For a binary formula, recursively convert the left- and right-hand sides.
        OpamPackageFormula::Binary { logop, lhs, rhs } => {
            let lhs_conv = parse_package_formula(lhs)?;
            let rhs_conv = parse_package_formula(rhs)?;
            let binary = Binary {
                lhs: Box::new(lhs_conv),
                rhs: Box::new(rhs_conv),
            };
            match logop {
                LogicalOp::And => Some(PackageFormula::And(binary)),
                LogicalOp::Or => Some(PackageFormula::Or(binary)),
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
            let base = PackageFormula::Base {
                name: s.clone(),
                range: HashedRange(Range::any()),
            };
            Some(base)
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

fn get_depends(formula: Option<DependsField>) -> Vec<OpamPackageFormula> {
    match formula {
        Some(DependsField::Multiple(vec)) => vec,
        Some(DependsField::Single(pf)) => vec![pf],
        None => vec![],
    }
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
    let dependencies = get_depends(opam_data.depends)
        .into_iter()
        .filter_map(|pf| parse_package_formula(&pf))
        .collect();
    Ok(dependencies)
}
