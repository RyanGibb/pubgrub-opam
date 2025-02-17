use pubgrub::error::PubGrubError;
use pubgrub::report::{DefaultStringReporter, Reporter};
use pubgrub::solver::Dependencies;
use pubgrub::solver::DependencyProvider;
use pubgrub::type_aliases::SelectedDependencies;
use pubgrub_opam::index::Index;
use pubgrub_opam::opam_deps::Package;
use pubgrub_opam::opam_version::OpamVersion;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::str::FromStr;

fn solve_repo(pkg: Package, version: OpamVersion, repo: &str) -> Result<(), Box<dyn Error>> {
    let index = Index::new(repo.to_string());

    let sol: SelectedDependencies<Package, OpamVersion> =
        match pubgrub::solver::resolve(&index, pkg, version) {
            Ok(sol) => sol,
            Err(PubGrubError::NoSolution(mut derivation_tree)) => {
                derivation_tree.collapse_no_versions();
                eprintln!("{}", DefaultStringReporter::report(&derivation_tree));
                panic!("failed to find a solution");
            }
            Err(err) => panic!("{:?}", err),
        };

    fn get_resolved_deps<'a>(
        index: &'a Index,
        sol: &'a SelectedDependencies<Package, OpamVersion>,
        package: Package,
        version: &'a OpamVersion,
    ) -> HashSet<(String, &'a OpamVersion)> {
        let dependencies = index.get_dependencies(&package, &version);
        match dependencies {
            Ok(Dependencies::Known(constraints)) => {
                let sol: &HashMap<
                    Package,
                    OpamVersion,
                    std::hash::BuildHasherDefault<rustc_hash::FxHasher>,
                > = &sol;
                let mut dependents = HashSet::new();
                for (dep_package, _dep_versions) in constraints {
                    let solved_version = sol.get(&dep_package).unwrap();
                    match dep_package {
                        Package::Base(name) => {
                            dependents.insert((name, solved_version));
                        }
                        Package::Lor { lhs : _, rhs : _ } => {
                            dependents.extend(get_resolved_deps(&index, sol, dep_package, solved_version));
                        }
                        Package::Proxy { name : _, formula : _ } => {
                            dependents.extend(get_resolved_deps(&index, sol, dep_package, solved_version));
                        }
                        Package::Var(_) => {
                            dependents.insert((format!("{}", dep_package), solved_version));
                        }
                    };
                }
                dependents
            }
            _ => {
                println!("No available dependencies for package {}", package);
                HashSet::new()
            }
        }
    }


    println!("\n\nSolution Set:");
    for (package, version) in &sol {
        match package {
            Package::Base(name) => {
                println!("\t({}, {})", name, version);
            }
            Package::Var(name) => {
                println!("\t{} = {}", name, version);
            }
            // Package::Lor { lhs, rhs } => {
            //     match version {
            //         OpamVersion(ver) => match ver.as_str() {
            //             "lhs" => println!("\t{}", lhs),
            //             "rhs" => println!("\t{}", rhs),
            //             _ => panic!("Unknown OR version {}", version),
            //         }
            //     }
            // }
            // Package::Proxy { name : _, formula : _ } => {
            //     println!("\t{}, {}", package, version);
            // }
            _ => ()
        }
    }

    let mut resolved_graph: HashMap<(String, &OpamVersion), HashSet<(String, &OpamVersion)>> =
        HashMap::new();
    for (package, version) in &sol {
        match package {
            Package::Base(name) => {
                let deps = get_resolved_deps(&index, &sol, package.clone(), version);
                resolved_graph.insert((name.clone(), version), deps);
            }
            _ => {}
        }
    }

    println!("\n\nResolved Dependency Graph:");
    for ((name, version), dependents) in resolved_graph {
        print!("\t({}, {})", name, version);
        if dependents.len() > 0 {
            print!(" -> ")
        }
        let mut first = true;
        for (dep_name, dep_version) in &dependents {
            if !first {
                print!(", ");
            }
            print!("({}, {})", dep_name, dep_version);
            first = false;
        }
        println!()
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    solve_repo(
        Package::from_str("A").unwrap(),
        "1.0.0".parse::<OpamVersion>().unwrap(),
        "./example-repo/packages",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_formulas_a100() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("A").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./package-formula-repo/packages",
        )
    }

    #[test]
    fn test_package_formulas_a110() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("A").unwrap(),
            "1.1.0".parse::<OpamVersion>().unwrap(),
            "./package-formula-repo/packages",
        )
    }

    #[test]
    fn test_package_formulas_a120() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("A").unwrap(),
            "1.2.0".parse::<OpamVersion>().unwrap(),
            "./package-formula-repo/packages",
        )
    }

    #[test]
    fn test_package_formulas_a130() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("A").unwrap(),
            "1.3.0".parse::<OpamVersion>().unwrap(),
            "./package-formula-repo/packages",
        )
    }

    #[test]
    fn test_package_formulas_a200() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("A").unwrap(),
            "2.0.0".parse::<OpamVersion>().unwrap(),
            "./package-formula-repo/packages",
        )
    }

    #[test]
    fn test_package_formulas_a210() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("A").unwrap(),
            "2.1.0".parse::<OpamVersion>().unwrap(),
            "./package-formula-repo/packages",
        )
    }

    #[test]
    fn test_package_formulas_a300() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("A").unwrap(),
            "3.0.0".parse::<OpamVersion>().unwrap(),
            "./package-formula-repo/packages",
        )
    }

    // TODO implement variables
    #[test]
    #[should_panic]
    fn test_filtered_package_formula_variables() -> () {
        let _ = solve_repo(
            Package::from_str("A").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./filtered-package-formula-repo/packages",
        );
        ()
    }

    #[test]
    fn test_filtered_package_formula_simple() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("D").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./filtered-package-formula-repo/packages",
        )
    }

    #[test]
    fn test_filtered_package_formula_complex() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("E").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./filtered-package-formula-repo/packages",
        )
    }

    #[test]
    fn test_opam_repository() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("dune").unwrap(),
            "3.17.2".parse::<OpamVersion>().unwrap(),
            "./opam-repository/packages",
        )
    }
}
