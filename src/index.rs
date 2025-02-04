use core::ops::{Bound, RangeBounds};
use pubgrub::type_aliases::Map;
use pubgrub::range::Range;
use pubgrub::version::Version;
use std::collections::BTreeMap;

use crate::opam_version::OpamVersion;

pub type PackageName = String;

pub struct Index {
    pub packages:
        Map<PackageName, BTreeMap<OpamVersion, Deps>>,
}

pub type Deps = Map<PackageName, Range<OpamVersion>>;

impl Index {
    /// Empty new index.
    pub fn new() -> Self {
        Self {
            packages: Map::default(),
        }
    }

    /// List existing versions for a given package with newest versions first.
    pub fn available_versions(&self, package: &PackageName) -> impl Iterator<Item = &OpamVersion> {
        self.packages
            .get(package)
            .into_iter()
            .flat_map(|k| k.keys())
            .rev()
    }

    /// Register a package and its mandatory dependencies in the index.
    pub fn add_deps<R: RangeBounds<OpamVersion>>(
        &mut self,
        package: &str,
        version: OpamVersion,
        new_deps: &[(&str, R)],
    ) {
        let deps = self
            .packages
            .entry(package.to_string())
            .or_default()
            .entry(version)
            .or_default();
        for (p, r) in new_deps {
            deps.insert(String::from(*p), range_from_bounds(r));
        }
    }
}

/// Convert a range bounds into pubgrub Range type.
pub fn range_from_bounds<R: RangeBounds<OpamVersion>>(bounds: &R) -> Range<OpamVersion> {
    match (bounds.start_bound(), bounds.end_bound()) {
        (Bound::Unbounded, Bound::Unbounded) => Range::any(),
        (Bound::Unbounded, Bound::Excluded(end)) => Range::strictly_lower_than(end.clone()),
        (Bound::Unbounded, Bound::Included(end)) => Range::strictly_lower_than(end.bump()),
        (Bound::Included(start), Bound::Unbounded) => Range::higher_than(start.clone()),
        (Bound::Included(start), Bound::Included(end)) => Range::between(start.clone(), end.bump()),
        (Bound::Included(start), Bound::Excluded(end)) => Range::between(start.clone(), end.clone()),
        (Bound::Excluded(start), Bound::Unbounded) => Range::higher_than(start.bump()),
        (Bound::Excluded(start), Bound::Included(end)) => Range::between(start.bump(), end.bump()),
        (Bound::Excluded(start), Bound::Excluded(end)) => Range::between(start.bump(), end.clone()),
    }
}
