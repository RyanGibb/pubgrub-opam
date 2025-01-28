// SPDX-License-Identifier: MPL-2.0
// https://github.com/pubgrub-rs/advanced_dependency_providers/

use crate::index::{Deps, Index};
use crate::opam_version::OpamVersion;
use core::borrow::Borrow;
use core::fmt::Display;
use pubgrub::range::Range;
use pubgrub::solver::{Dependencies, DependencyConstraints, DependencyProvider};
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Package {
    Base(String),
}

impl Package {
    fn base_pkg(&self) -> &String {
        match self {
            Package::Base(pkg) => pkg,
        }
    }
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
        }
    }
}

impl Index {
    pub fn list_versions(&self, package: &Package) -> impl Iterator<Item = &OpamVersion> {
        self.available_versions(package.base_pkg())
    }
}

impl DependencyProvider<Package, OpamVersion> for Index {
    fn choose_package_version<T: Borrow<Package>, U: Borrow<Range<OpamVersion>>>(
        &self,
        potential_packages: impl Iterator<Item = (T, U)>,
    ) -> Result<(T, Option<OpamVersion>), Box<dyn std::error::Error>> {
        Ok(pubgrub::solver::choose_package_with_fewest_versions(
            |p| self.list_versions(p).cloned(),
            potential_packages,
        ))
    }

    fn get_dependencies(
        &self,
        package: &Package,
        version: &OpamVersion,
    ) -> Result<Dependencies<Package, OpamVersion>, Box<dyn std::error::Error>> {
        let all_versions = match self.packages.get(package.base_pkg()) {
            None => return Ok(Dependencies::Unknown),
            Some(all_versions) => all_versions,
        };
        let deps = match all_versions.get(version) {
            None => return Ok(Dependencies::Unknown),
            Some(deps) => deps,
        };

        match package {
            Package::Base(_) => Ok(Dependencies::Known(from_deps(deps.clone()))),
        }
    }
}

fn from_deps(deps: Deps) -> DependencyConstraints<Package, OpamVersion> {
    deps.iter()
        .map(|(base_pkg, dep)| {
            (Package::Base(base_pkg.clone()), dep.clone())
        })
        .collect()
}
