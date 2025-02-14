use crate::index::{Binary, HashedRange, PackageFormula, VersionFormula};
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
    // TODO implement logic for this is it's actually used anywhere
    // Defined,
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
    // TODO implement logic for these if they're actually used anywhere
    // Int(i64),
    // Bool(bool),
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
        pfxop: UnaryOp,
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

fn parse_filter_expr(filter: &FilterExpr) -> VersionFormula {
    match filter {
        FilterExpr::LogOp { logop, lhs, rhs} => {
            let left = parse_filter_expr(lhs);
            let right = parse_filter_expr(rhs);
            match logop {
                LogicalOp::And =>
                    match (left.clone(), right.clone()) {
                        (VersionFormula::Version(l), VersionFormula::Version(r)) => VersionFormula::Version(HashedRange(l.0.intersection(&r.0))),
                        _ => VersionFormula::Or( Binary { lhs: Box::new(left), rhs: Box::new(right)} )
                    },
                LogicalOp::Or =>
                    match (left.clone(), right.clone()) {
                        (VersionFormula::Version(l), VersionFormula::Version(r)) => VersionFormula::Version(HashedRange(l.0.union(&r.0))),
                        _ => VersionFormula::And( Binary { lhs: Box::new(left), rhs: Box::new(right)} )
                    },
            }
        },
        FilterExpr::Unary {pfxop, arg} => {
            match pfxop {
                UnaryOp::Not => {
                    let inner = parse_filter_expr(*&arg);
                    match inner {
                        VersionFormula::Version(version) => {
                            VersionFormula::Version(HashedRange(version.0.negate()))
                        },
                        _ => VersionFormula::Not(Box::new(inner)),
                    }
                },
            }
        },
        FilterExpr::Group { group } => {
            if group.is_empty() {
                panic!("Empty group");
            } else {
                parse_filter_expr(&group[0])
            }
        },
        FilterExpr::Relop { relop, lhs, rhs } => {
            let left = parse_filter_expr(lhs);
            let right = parse_filter_expr(rhs);
            match relop {
                RelOp::Eq => VersionFormula::Eq( Binary { lhs: Box::new(left), rhs: Box::new(right)} ),
                RelOp::Geq => VersionFormula::Geq ( Binary { lhs: Box::new(left), rhs: Box::new(right)} ),
                RelOp::Gt => VersionFormula::Gt ( Binary { lhs: Box::new(left), rhs: Box::new(right)} ),
                RelOp::Lt => VersionFormula::Lt ( Binary { lhs: Box::new(left), rhs: Box::new(right)} ),
                RelOp::Leq => VersionFormula::Leq ( Binary { lhs: Box::new(left), rhs: Box::new(right)} ),
                RelOp::Neq => VersionFormula::Neq ( Binary { lhs: Box::new(left), rhs: Box::new(right)} ),
            }
        }
         FilterExpr::Variable { id } => VersionFormula::Variable(id.to_string()),
         FilterExpr::Literal(lit) =>
            match lit {
                LiteralValue::Str(s) => {
                    let version = s.parse::<OpamVersion>().unwrap();
                    let range = Range::<OpamVersion>::exact(version);
                    VersionFormula::Version(HashedRange(range))
                }
            }
    }
}

fn parse_version_formula(formula: &OpamVersionFormula) -> VersionFormula {
    match formula {
        OpamVersionFormula::LogOp { logop, lhs, rhs } => {
            let left = parse_version_formula(lhs);
            let right = parse_version_formula(rhs);
            match logop {
                LogicalOp::And =>
                    match (left.clone(), right.clone()) {
                        (VersionFormula::Version(l), VersionFormula::Version(r)) => VersionFormula::Version(HashedRange(l.0.intersection(&r.0))),
                        _ => VersionFormula::Or( Binary { lhs: Box::new(left), rhs: Box::new(right)} )
                    },
                LogicalOp::Or =>
                    match (left.clone(), right.clone()) {
                        (VersionFormula::Version(l), VersionFormula::Version(r)) => VersionFormula::Version(HashedRange(l.0.union(&r.0))),
                        _ => VersionFormula::And( Binary { lhs: Box::new(left), rhs: Box::new(right)} )
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
                    VersionFormula::Version(HashedRange(range))
                },
                FilterOrVersion::Filter(filter) => parse_filter_expr(filter),
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
                    let inner = parse_version_formula(*&arg);
                    match inner {
                        VersionFormula::Version(version) => {
                            VersionFormula::Version(HashedRange(version.0.negate()))
                        },
                        _ => VersionFormula::Not(Box::new(inner)),
                    }
                },
            }
        }
        OpamVersionFormula::Filter(filter) => parse_filter_expr(filter),
    }
}

pub fn parse_package_formula(formula: &OpamPackageFormula) -> PackageFormula {
    match formula {
        OpamPackageFormula::Simple { name, conditions } => {
            let formula = if conditions.is_empty() {
                VersionFormula::Version(HashedRange(Range::any()))
            } else {
                parse_version_formula(&conditions[0])
            };
            PackageFormula::Base {
                name: name.clone(),
                formula,
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
        },
        OpamPackageFormula::Group { group } => {
            if group.is_empty() {
                panic!("Empty group");
            } else {
                parse_package_formula(&group[0])
            }
        },
        OpamPackageFormula::Plain(s) => {
            PackageFormula::Base {
                name: s.clone(),
                formula: VersionFormula::Version(HashedRange(Range::any())),
            }
        },
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
        .map(|pf| parse_package_formula(&pf))
        .collect();
    Ok(dependencies)
}
