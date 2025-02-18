use pubgrub::error::PubGrubError;
use pubgrub::report::{DefaultStringReporter, Reporter};
use pubgrub::solver::Dependencies;
use pubgrub::solver::DependencyProvider;
use pubgrub::type_aliases::SelectedDependencies;
use pubgrub_opam::index::Index;
use pubgrub_opam::opam_deps::Package;
use pubgrub_opam::opam_version::OpamVersion;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::str::FromStr;

fn solve_repo(pkg: Package, version: OpamVersion, repo: &str) -> Result<SelectedDependencies<Package, OpamVersion>, Box<dyn Error>> {
    let index = Index::new(repo.to_string());
    index.set_debug(true);

    let sol: SelectedDependencies<Package, OpamVersion> =
        match pubgrub::solver::resolve(&index, pkg, version) {
            Ok(sol) => Ok(sol),
            Err(PubGrubError::NoSolution(mut derivation_tree)) => {
                derivation_tree.collapse_no_versions();
                eprintln!("{}", DefaultStringReporter::report(&derivation_tree));
                Err(PubGrubError::NoSolution(derivation_tree))
            }
            Err(err) => panic!("{:?}", err),
        }?;

    index.set_debug(false);

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

    println!("\nSolution Set:");
    for (package, version) in &sol {
        match package {
            Package::Base(name) => {
                println!("\t({}, {})", name, version);
            }
            Package::Var(name) => {
                println!("\t{} = {}", name, version);
            }
            _ => ()
        }
    }

    let mut resolved_graph: BTreeMap<(String, &OpamVersion), Vec<(String, &OpamVersion)>> =
        BTreeMap::new();
    for (package, version) in &sol {
        match package {
            Package::Base(name) => {
                let mut deps = get_resolved_deps(&index, &sol, package.clone(), version).into_iter().collect::<Vec<_>>();
                deps.sort_by(|(p1, _v1), (p2, _v2)| p1.cmp(p2));
                resolved_graph.insert((name.clone(), version), deps);
            }
            _ => {}
        }
    }

    println!("\nResolved Dependency Graph:");
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

    Ok(sol)
}

fn main() -> Result<(), Box<dyn Error>> {
    let _ = solve_repo(
        Package::from_str("A").unwrap(),
        "1.0.0".parse::<OpamVersion>().unwrap(),
        "./example-repo/packages",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_solve() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("A").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        Ok(())
    }

    #[test]
    fn test_simple_error() -> Result<(), Box<dyn Error>> {
        let result = solve_repo(
            Package::from_str("A").unwrap(),
            "2.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        );
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_package_formula() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("package-formula").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        Ok(())
    }

    #[test]
    fn test_package_formula_and() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("package-formula-and").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        Ok(())
    }

    #[test]
    fn test_package_formula_and_error() -> Result<(), Box<dyn Error>> {
        let result = solve_repo(
            Package::from_str("package-formula-and-error").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        );
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn test_package_formula_or() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("package-formula-or").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        Ok(())
    }

    #[test]
    fn test_package_formula_or2() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("package-formula-or").unwrap(),
            "2.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        Ok(())
    }

    #[test]
    fn test_package_formula_or3() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("package-formula-or").unwrap(),
            "3.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        Ok(())
    }

    #[test]
    fn test_package_formula_or_error() -> Result<(), Box<dyn Error>> {
        let result = solve_repo(
            Package::from_str("package-formula-or-error").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        );
        assert!(result.is_err());
        Ok(())
    }


    #[test]
    fn test_package_formula_and_or() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("package-formula-and-or").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        Ok(())
    }

    #[test]
    fn test_filtered_package_formula_variable() -> Result<(), Box<dyn Error>> {
        let sol = solve_repo(
            Package::from_str("filtered-package-formula-variable").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        assert_eq!(sol.get(&Package::from_str("D").unwrap()), Some("2.0.0".parse::<OpamVersion>().as_ref().unwrap()));
        Ok(())
    }

    #[test]
    fn test_filtered_package_formula_and_variable() -> Result<(), Box<dyn Error>> {
        let sol = solve_repo(
            Package::from_str("filtered-package-formula-and-variable").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        assert_eq!(sol.get(&Package::Base("A".to_string())), Some("1.0.0".parse::<OpamVersion>().as_ref().unwrap()));
        assert_eq!(sol.get(&Package::Var("test".to_string())), Some("false".parse::<OpamVersion>().as_ref().unwrap()));
        assert_eq!(sol.get(&Package::Var("build".to_string())), Some("false".parse::<OpamVersion>().as_ref().unwrap()));
        Ok(())
    }

    #[test]
    fn test_filtered_package_formula_variable_version() -> Result<(), Box<dyn Error>> {
        let sol = solve_repo(
            Package::from_str("filtered-package-formula-variable-version").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        assert_eq!(sol.get(&Package::from_str("D").unwrap()), Some("3.0.0".parse::<OpamVersion>().as_ref().unwrap()));
        Ok(())
    }

    // TODO look at and encoding
    #[test]
    fn test_filtered_package_formula_and() -> Result<(), Box<dyn Error>> {
        let sol = solve_repo(
            Package::from_str("filtered-package-formula-and").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        assert_eq!(sol.get(&Package::from_str("A").unwrap()), Some("1.0.0".parse::<OpamVersion>().as_ref().unwrap()));
        Ok(())
    }

    // TODO look at and encoding
    #[test]
    fn test_filtered_package_formula_and_error() -> Result<(), Box<dyn Error>> {
        let result = solve_repo(
            Package::from_str("filtered-package-formula-and-error").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        );
        assert!(result.is_err());
        Ok(())
    }

    // TODO look at or encoding
    #[test]
    fn test_filtered_package_formula_or() -> Result<(), Box<dyn Error>> {
        let sol = solve_repo(
            Package::from_str("filtered-package-formula-or").unwrap(),
            "1.0.0".parse::<OpamVersion>().unwrap(),
            "./example-repo/packages",
        )?;
        assert_eq!(sol.get(&Package::from_str("A").unwrap()), Some("1.0.0".parse::<OpamVersion>().as_ref().unwrap()));
        Ok(())
    }

    #[test]
    fn test_opam_repository_dune() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("dune").unwrap(),
            "3.17.2".parse::<OpamVersion>().unwrap(),
            "./opam-repository/packages",
        )?;
        Ok(())
    }

    #[test]
    fn test_opam_repository_ocaml_variants() -> Result<(), Box<dyn Error>> {
        solve_repo(
            Package::from_str("ocaml-variants").unwrap(),
            "5.3.1+trunk".parse::<OpamVersion>().unwrap(),
            "./opam-repository/packages",
        )?;
        Ok(())
    }
}
