use crate::index::{Binary, Index, PackageFormula, VersionFormula};
use crate::opam_version::OpamVersion;
use crate::parse::parse_dependencies_for_package_version;
use core::borrow::Borrow;
use core::fmt::Display;
use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use pubgrub::range::Range;
use pubgrub::solver::{Dependencies, DependencyConstraints, DependencyProvider};
use pubgrub::version::Version;
use std::str::FromStr;
use pubgrub::type_aliases::Map;


#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Package {
    Base(String),
    Lor {
        lhs: Box<PackageFormula>,
        rhs: Box<PackageFormula>,
    },
    Proxy {
        name: String,
        formula: Box<VersionFormula>
    },
    Var(String)
}

static VARIABLE_CACHE: LazyLock<Mutex<HashMap<String, HashSet<OpamVersion>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

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
            Package::Lor { lhs, rhs } => write!(f, "{} | {}", lhs, rhs),
            Package::Proxy { name, formula } => write!(f, "{} {{{}}}", name, formula),
            Package::Var(var) => write!(f, "`{}`", var),
        }
    }
}

static LHS_VERSION: LazyLock<OpamVersion> = LazyLock::new(|| OpamVersion("lhs".to_string()));
static RHS_VERSION: LazyLock<OpamVersion> = LazyLock::new(|| OpamVersion("rhs".to_string()));

static TRUE_VERSION: LazyLock<OpamVersion> = LazyLock::new(|| OpamVersion("true".to_string()));
static FALSE_VERSION: LazyLock<OpamVersion> = LazyLock::new(|| OpamVersion("false".to_string()));

impl Index {
    pub fn list_versions(&self, package: &Package) -> Box<Vec<OpamVersion>> {
        // println!("list {}", package);
        let versions = match package {
            Package::Base(pkg) => self.available_versions(pkg),
            Package::Var(var) =>
                match VARIABLE_CACHE.lock().unwrap().get(var) {
                    Some(m) => m.iter().cloned().collect(),
                    None => vec![FALSE_VERSION.clone(), TRUE_VERSION.clone()],
                },
            _ => {
                vec![LHS_VERSION.clone(), RHS_VERSION.clone()]
            },
        };
        // println!("\t{:?}", versions);
        Box::new(versions)
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
                let formulas = parse_dependencies_for_package_version(self.repo.as_str(), pkg, version.to_string().as_str()).unwrap();
                let deps = from_formulas(&formulas);
                if self.debug.get() {
                    print!("({}, {})", package, version);
                    if deps.len() > 0 {
                        print!(" -> ")
                    }
                    let mut first = true;
                    for (package, range) in deps.clone() {
                        if !first {
                            print!(", ");
                        }
                        print!("({}, {})", package, range);
                        first = false;
                    }
                    println!();
                }
                Ok(Dependencies::Known(deps))
            }
            Package::Lor { lhs, rhs } => {
                let deps = match version {
                    OpamVersion(ver) => match ver.as_str() {
                        "lhs" => from_formula(*&lhs),
                        "rhs" => from_formula(*&rhs),
                        _ => panic!("Unknown OR version {}", version),
                    }
                };
                if self.debug.get() {
                    print!("({}, {})", package, version);
                    if deps.len() > 0 {
                        print!(" -> ")
                    }
                    let mut first = true;
                    for (package, range) in deps.clone() {
                        if !first {
                            print!(", ");
                        }
                        print!("({}, {})", package, range);
                        first = false;
                    }
                    println!();
                }
                Ok(Dependencies::Known(deps))
            }
            Package::Proxy { name, formula } => {
                let deps = from_version_formula(name, version, formula);
                if self.debug.get() {
                    print!("({}, {})", package, version);
                    if deps.len() > 0 {
                        print!(" -> ")
                    }
                    let mut first = true;
                    for (package, range) in deps.clone() {
                        if !first {
                            print!(", ");
                        }
                        print!("({}, {})", package, range);
                        first = false;
                    }
                    println!();
                }
                Ok(Dependencies::Known(deps))
            }
            Package::Var(_) => {
                Ok(Dependencies::Known(Map::default()))
            }
        }
    }
}

fn from_version_formula(name: &String, version: &OpamVersion, formula: &VersionFormula) -> DependencyConstraints<Package, OpamVersion> {
    let mut map = Map::default();
    match formula {
        VersionFormula::Version(range) => {
            map.insert(Package::Base(name.to_string()), range.0.clone());
            map
        },
        VersionFormula::Variable(variable) => {
            match version {
                OpamVersion(ver) => match ver.as_str() {
                    "lhs" => {
                        map.insert(Package::Var(variable.to_string()), Range::exact(FALSE_VERSION.clone()));
                        ()
                    },
                    "rhs" => {
                        map.insert(Package::Base(name.to_string()), Range::any());
                        map.insert(Package::Var(variable.to_string()), Range::exact(TRUE_VERSION.clone()));
                    },
                    _ => panic!("Unknown Proxy version {}", version),
                }
            };
            map
        },
        VersionFormula::Not(variable) => {
            match version {
                OpamVersion(ver) => match ver.as_str() {
                    "lhs" => {
                        map.insert(Package::Base(name.to_string()), Range::any());
                        map.insert(Package::Var(variable.to_string()), Range::exact(FALSE_VERSION.clone()));
                    },
                    "rhs" => {
                        map.insert(Package::Var(variable.to_string()), Range::exact(TRUE_VERSION.clone()));
                        ()
                    }
                    _ => panic!("Unknown Proxy version {}", version),
                }
            };
            map
        }
        VersionFormula::Eq(Binary { lhs, rhs }) => {
            match version {
                OpamVersion(ver) => match ver.as_str() {
                    "lhs" => match (*lhs.clone(), *rhs.clone()) {
                        (VersionFormula::Lit(ver), VersionFormula::Variable(var)) => {
                            VARIABLE_CACHE.lock().unwrap()
                                                 .entry(var.to_string())
                                                 .or_insert_with(HashSet::new)
                                                 .insert(ver.clone());
                            map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::exact(ver).negate())
                        },
                        (VersionFormula::Variable(var), VersionFormula::Lit(ver)) => {
                            VARIABLE_CACHE.lock().unwrap()
                                                 .entry(var.to_string())
                                                 .or_insert_with(HashSet::new)
                                                 .insert(ver.clone());
                            map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::exact(ver).negate())
                        },
                        _ => panic!("invalid operator for ({}, {}): {}", name, version, formula)
                    }
                    "rhs" => map.insert(Package::Base(name.to_string()), Range::any()),
                    _ => panic!("Unknown Proxy version {}", version),
                }
            };
            map
        }
        VersionFormula::Neq(Binary { lhs, rhs }) => {
            match version {
                OpamVersion(ver) => match ver.as_str() {
                    "lhs" => match (*lhs.clone(), *rhs.clone()) {
                        (VersionFormula::Lit(ver), VersionFormula::Variable(var)) => map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::exact(ver)),
                        (VersionFormula::Variable(var), VersionFormula::Lit(ver)) => map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::exact(ver)),
                        _ => panic!("invalid operator for ({}, {}): {}", name, version, formula)
                    }
                    "rhs" => map.insert(Package::Base(name.to_string()), Range::any()),
                    _ => panic!("Unknown Proxy version {}", version),
                }
            };
            map
        },
        VersionFormula::Geq(Binary { lhs, rhs }) => {
            match version {
                OpamVersion(ver) => match ver.as_str() {
                    "lhs" => match (*lhs.clone(), *rhs.clone()) {
                        (VersionFormula::Lit(ver), VersionFormula::Variable(var)) => {
                            VARIABLE_CACHE.lock().unwrap()
                                                 .entry(var.to_string())
                                                 .or_insert_with(HashSet::new)
                                                 .insert(ver.clone());
                            map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::strictly_lower_than(ver))
                        },
                        (VersionFormula::Variable(var), VersionFormula::Lit(ver)) => {
                            VARIABLE_CACHE.lock().unwrap()
                                                 .entry(var.to_string())
                                                 .or_insert_with(HashSet::new)
                                                 .insert(ver.clone());
                            map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::strictly_lower_than(ver))
                        },
                        _ => panic!("invalid operator for ({}, {}): {}", name, version, formula)
                    }
                    "rhs" => map.insert(Package::Base(name.to_string()), Range::any()),
                    _ => panic!("Unknown Proxy version {}", version),
                }
            };
            map
        },
        VersionFormula::Gt(Binary { lhs, rhs }) => {
            match version {
                OpamVersion(ver) => match ver.as_str() {
                    "lhs" => match (*lhs.clone(), *rhs.clone()) {
                        (VersionFormula::Lit(ver), VersionFormula::Variable(var)) => {
                            VARIABLE_CACHE.lock().unwrap()
                                                 .entry(var.to_string())
                                                 .or_insert_with(HashSet::new)
                                                 .insert(ver.clone());
                            map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::strictly_lower_than(ver.bump()))
                        },
                        (VersionFormula::Variable(var), VersionFormula::Lit(ver)) => {
                            VARIABLE_CACHE.lock().unwrap()
                                                 .entry(var.to_string())
                                                 .or_insert_with(HashSet::new)
                                                 .insert(ver.clone());
                            map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::strictly_lower_than(ver.bump()))
                        },
                        _ => panic!("invalid operator for ({}, {}): {}", name, version, formula)
                    }
                    "rhs" => map.insert(Package::Base(name.to_string()), Range::any()),
                    _ => panic!("Unknown Proxy version {}", version),
                }
            };
            map
        },
        VersionFormula::Leq(Binary { lhs, rhs }) => {
            match version {
                OpamVersion(ver) => match ver.as_str() {
                    "lhs" => match (*lhs.clone(), *rhs.clone()) {
                        (VersionFormula::Lit(ver), VersionFormula::Variable(var)) => {
                            VARIABLE_CACHE.lock().unwrap()
                                                 .entry(var.to_string())
                                                 .or_insert_with(HashSet::new)
                                                 .insert(ver.clone());
                            map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::higher_than(ver.bump()))
                        },
                        (VersionFormula::Variable(var), VersionFormula::Lit(ver)) => {
                            VARIABLE_CACHE.lock().unwrap()
                                                 .entry(var.to_string())
                                                 .or_insert_with(HashSet::new)
                                                 .insert(ver.clone());
                            map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::higher_than(ver.bump()))
                        },
                        _ => panic!("invalid operator for ({}, {}): {}", name, version, formula)
                    }
                    "rhs" => map.insert(Package::Base(name.to_string()), Range::any()),
                    _ => panic!("Unknown Proxy version {}", version),
                }
            };
            map
        },
        VersionFormula::Lt(Binary { lhs, rhs }) => {
            match version {
                OpamVersion(ver) => match ver.as_str() {
                    "lhs" => match (*lhs.clone(), *rhs.clone()) {
                        (VersionFormula::Lit(ver), VersionFormula::Variable(var)) => {
                            VARIABLE_CACHE.lock().unwrap()
                                                 .entry(var.to_string())
                                                 .or_insert_with(HashSet::new)
                                                 .insert(ver.clone());
                            map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::higher_than(ver))
                        },
                        (VersionFormula::Variable(var), VersionFormula::Lit(ver)) => {
                            VARIABLE_CACHE.lock().unwrap()
                                                 .entry(var.to_string())
                                                 .or_insert_with(HashSet::new)
                                                 .insert(ver.clone());
                            map.insert(Package::Var(var.to_string()), Range::<OpamVersion>::higher_than(ver))
                        },
                        _ => panic!("invalid operator for ({}, {}): {}", name, version, formula)
                    }
                    "rhs" => map.insert(Package::Base(name.to_string()), Range::any()),
                    _ => panic!("Unknown Proxy version {}", version),
                }
            };
            map
        },
        VersionFormula::And(Binary { lhs, rhs }) => match version {
            OpamVersion(ver) => match ver.as_str() {
                "lhs" => map,
                "rhs" => {
                    let left = from_version_formula(name, version, lhs);
                    let right = from_version_formula(name, version, rhs);
                    merge_constraints(left, right)
                }
                _ => panic!("Unknown Proxy version {}", version),
            }
        }
        VersionFormula::Or(Binary { lhs, rhs }) => match version {
            OpamVersion(ver) => match ver.as_str() {
                "lhs" => from_version_formula(name, version, lhs),
                "rhs" => from_version_formula(name, version, rhs),
                _ => panic!("Unknown Proxy version {}", version),
            }
        }
        _ => panic!("invalid literal for ({}, {}): {}", name, version, formula)
    }
}

pub fn from_formulas(formulas: &Vec<PackageFormula>) -> DependencyConstraints<Package, OpamVersion> {
    formulas.iter()
        .map(|formula| from_formula(formula))
        .fold(Map::default(), |acc, cons| merge_constraints(acc, cons))
}

fn from_formula(formula: &PackageFormula) -> DependencyConstraints<Package, OpamVersion> {
    match formula {
        PackageFormula::Base { name, formula } => {
            let mut map = Map::default();
            match formula {
                VersionFormula::Version(range) =>
                    map.insert(Package::Base(name.to_string()), range.0.clone()),
                _ =>
                    map.insert(Package::Proxy { name: name.to_string(), formula: Box::new(formula.clone()) }, Range::any()),
            };
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
