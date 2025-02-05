use pubgrub::error::PubGrubError;
use pubgrub::report::{DefaultStringReporter, Reporter};
use pubgrub::solver::Dependencies;
use pubgrub::solver::DependencyProvider;
use pubgrub::type_aliases::SelectedDependencies;
use pubgrub_opam::opam_deps::Package;
use pubgrub_opam::opam_version::OpamVersion;
use pubgrub_opam::parse::parse_repo;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;

fn main() -> Result<(), Box<dyn Error>> {
    let index = parse_repo("./example-repo/packages")?;

    println!("Created index with {} packages:", index.packages.len());

    for (name, version_map) in &index.packages {
        for (version, dependents) in version_map {
            print!("({}, {})", name, version);
            if dependents.len() > 0 {
                print!(" -> ")
            }
            let mut first = true;
            for formula in dependents {
                if !first {
                    print!(", ");
                }
                print!("{}", formula);
                first = false;
            }
            println!()
        }
    }

    let pkg = Package::from_str("A").unwrap();
    let sol: SelectedDependencies<Package, OpamVersion> =
        match pubgrub::solver::resolve(&index, pkg, "1.0.0".parse::<OpamVersion>().unwrap()) {
            Ok(sol) => sol,
            Err(PubGrubError::NoSolution(mut derivation_tree)) => {
                derivation_tree.collapse_no_versions();
                eprintln!("{}", DefaultStringReporter::report(&derivation_tree));
                panic!("failed to find a solution");
            }
            Err(err) => panic!("{:?}", err),
        };

    let mut resolved_graph: HashMap<_, Vec<_>> = HashMap::new();
    for (package, version) in &sol {
        let dependencies = index.get_dependencies(&package, &version);
        match dependencies {
            Ok(Dependencies::Known(constraints)) => {
                let sol: &HashMap<
                    Package,
                    OpamVersion,
                    std::hash::BuildHasherDefault<rustc_hash::FxHasher>,
                > = &sol;
                let mut dependents = Vec::new();
                for (dep_package, _dep_versions) in constraints {
                    let solved_version = sol.get(&dep_package).unwrap();
                    match dep_package {
                        Package::Base(name) => {
                            dependents.push((name, solved_version))
                        },
                        _ => {}
                    };
                }
                match package {
                    Package::Base(name) => {
                        resolved_graph.insert((name.clone(), version), dependents);
                    },
                    _ => {}
                }
            }
            _ => {
                println!("No available dependencies for package {}", package);
            }
        }
    }

    println!("Resolved Dependency Graph:");
    for ((name, version), dependents) in resolved_graph {
        print!("({}, {})", name, version);
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
