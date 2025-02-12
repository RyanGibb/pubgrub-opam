use crate::index::{Binary, Index, PackageFormula};
use crate::opam_version::OpamVersion;
use crate::parse::parse_dependencies_for_package_version;
use core::borrow::Borrow;
use core::fmt::Display;
use std::sync::LazyLock;
use pubgrub::range::Range;
use pubgrub::solver::{Dependencies, DependencyConstraints, DependencyProvider};
use std::str::FromStr;
use pubgrub::type_aliases::Map;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Package {
    Base(String),
    Lor{
        lhs: Box<PackageFormula>,
        rhs: Box<PackageFormula>,
    },
}

impl FromStr for Package {
    type Err = String;
    fn from_str(pkg: &str) -> Result<Self, Self::Err> {
        let mut pkg_parts = pkg.split('/');
        match (pkg_parts.next(), pkg_parts.next()) {
            (Some(base), None) => Ok(Package::Base(base.to_string())),
            _ => Err(format!("{} is not a valid package name", pkg)),
        }
    }
}

impl Display for Package {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Package::Base(pkg) => write!(f, "{}", pkg),
            Package::Lor { lhs, rhs } => write!(f, "({} | {})", lhs, rhs),
        }
    }
}

static LHS_VERSION: LazyLock<OpamVersion> = LazyLock::new(|| OpamVersion("lhs".to_string()));
static RHS_VERSION: LazyLock<OpamVersion> = LazyLock::new(|| OpamVersion("rhs".to_string()));

impl Index {
    pub fn list_versions(&self, package: &Package) -> Box<Vec<OpamVersion>> {
        // println!("list {}", package);
        match package {
            Package::Base(pkg) => {
                // println!("\t{:?}", self.available_versions(pkg));
                Box::new(self.available_versions(pkg))
            },
            Package::Lor { lhs : _, rhs : _} => {
                let versions = vec![LHS_VERSION.clone(), RHS_VERSION.clone()];
                Box::new(versions)
            },
        }
    }
}

impl DependencyProvider<Package, OpamVersion> for Index {
    fn choose_package_version<T: Borrow<Package>, U: Borrow<Range<OpamVersion>>>(
        &self,
        potential_packages: impl Iterator<Item = (T, U)>,
    ) -> Result<(T, Option<OpamVersion>), Box<dyn std::error::Error>> {
        Ok(pubgrub::solver::choose_package_with_fewest_versions(
            |p| self.list_versions(p).into_iter(),
            potential_packages,
        ))
    }

    fn get_dependencies(
        &self,
        package: &Package,
        version: &OpamVersion,
    ) -> Result<Dependencies<Package, OpamVersion>, Box<dyn std::error::Error>> {
        match package {
            Package::Base(pkg) => {
                print!("({}, {})", package, version);
                let deps = parse_dependencies_for_package_version(self.repo.as_str(), pkg, version.to_string().as_str()).unwrap();
                if deps.len() > 0 {
                    print!(" -> ")
                }
                let mut first = true;
                for formula in deps.clone() {
                    if !first {
                        print!(", ");
                    }
                    print!("{}", formula);
                    first = false;
                }
                println!();
                Ok(Dependencies::Known(from_formulas(&deps)))
            }
            Package::Lor { lhs, rhs } => {
                match version {
                    OpamVersion(ver) => match ver.as_str() {
                        "lhs" => Ok(Dependencies::Known(from_formula(*&lhs))),
                        "rhs" => Ok(Dependencies::Known(from_formula(*&rhs))),
                        _ => panic!("Unknown OR version {}", version),
                    }
                }
            }
        }
    }
}

pub fn from_formulas(formulas: &Vec<PackageFormula>) -> DependencyConstraints<Package, OpamVersion> {
    formulas.iter()
        .map(|formula| from_formula(formula))
        .fold(Map::default(), |acc, cons| merge_constraints(acc, cons))
}

fn from_formula(formula: &PackageFormula) -> DependencyConstraints<Package, OpamVersion> {
    match formula {
        PackageFormula::Base { name, range } => {
            let mut map = Map::default();
            map.insert(Package::Base(name.to_string()), range.0.clone());
            map
        },
        PackageFormula::Or(Binary { lhs, rhs }) => {
            let mut map = Map::default();
            map.insert(Package::Lor { lhs: lhs.clone(), rhs: rhs.clone() }, Range::any());
            map
        },
        PackageFormula::And(Binary { lhs, rhs }) => {
            let left = from_formula(lhs);
            let right = from_formula(rhs);
            merge_constraints(left, right)
        },
    }
}

fn merge_constraints(
    mut left: DependencyConstraints<Package, OpamVersion>,
    right: DependencyConstraints<Package, OpamVersion>,
) -> DependencyConstraints<Package, OpamVersion> {
    for (pkg, range) in right {
        left.entry(pkg)
            .and_modify(|existing| {
                *existing = existing.union(&range);
            })
            .or_insert(range);
    }
    left
}
