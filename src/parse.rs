use crate::index::Index;
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::ops::{Bound, RangeBounds};
use std::ops::{Bound::Excluded, Bound::Included, Bound::Unbounded};
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
pub struct OpamJson {
    #[serde(rename = "opam-version")]
    pub opam_version: Option<String>,
    pub name: String,
    pub version: String,
    pub depends: Option<Vec<OpamDependencyJson>>,
}

#[derive(Debug, Deserialize)]
pub struct OpamDependencyJson {
    pub val: String,
    pub conditions: Vec<ConditionJson>,
}

// a single version formula (e.g. `{"prefix_relop":"eq","arg":"1"}`)
// TODO package formula and recursive version formula
// https://opam.ocaml.org/doc/Manual.html#Package-Formulas
// this might get complicated
#[derive(Debug, Deserialize)]
pub struct ConditionJson {
    #[serde(rename = "prefix_relop")]
    pub relop: Option<String>,
    pub arg: Option<String>,
}

#[derive(Debug)]
struct SimpleRange {
    start: Bound<u32>,
    end: Bound<u32>,
}

impl RangeBounds<u32> for SimpleRange {
    fn start_bound(&self) -> Bound<&u32> {
        match &self.start {
            Included(x) => Included(x),
            Excluded(x) => Excluded(x),
            Unbounded => Unbounded,
        }
    }
    fn end_bound(&self) -> Bound<&u32> {
        match &self.end {
            Included(x) => Included(x),
            Excluded(x) => Excluded(x),
            Unbounded => Unbounded,
        }
    }
}

fn condition_to_simple_range(cond: &ConditionJson) -> Option<SimpleRange> {
    let relop = cond.relop.as_deref()?;
    let arg_str = cond.arg.as_deref()?;
    let val = arg_str.parse::<u32>().ok()?;

    match relop {
        "eq" => Some(SimpleRange {
            start: Included(val),
            end: Excluded(val + 1),
        }),
        "geq" => Some(SimpleRange {
            start: Included(val),
            end: Unbounded,
        }),
        "gt" => Some(SimpleRange {
            start: Excluded(val),
            end: Unbounded,
        }),
        "leq" => Some(SimpleRange {
            start: Unbounded,
            end: Included(val),
        }),
        "lt" => Some(SimpleRange {
            start: Unbounded,
            end: Excluded(val),
        }),
        // TODO neq
        _ => None,
    }
}

fn dependency_bounds(dep: &OpamDependencyJson) -> SimpleRange {
    let mut result = SimpleRange {
        start: Unbounded,
        end: Unbounded,
    };
    for cond in &dep.conditions {
        if let Some(range) = condition_to_simple_range(cond) {
            // TODO combine conditions
            result = range;
        }
    }
    result
}

pub fn parse_repo(repo_path: &str) -> Result<Index, Box<dyn Error>> {
    let mut index = Index::new();
    for entry in WalkDir::new(repo_path).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() && entry.file_name() == "opam.json" {
            let content = fs::read_to_string(entry.path())?;
            let opam_data: OpamJson = serde_json::from_str(&content)?;

            // TODO proper versions
            // https://opam.ocaml.org/doc/Manual.html#Version-ordering
            let pkg_version = opam_data.version.parse::<u32>()?;

            let mut deps_array = Vec::new();
            if let Some(depends_list) = &opam_data.depends {
                for dep in depends_list {
                    let dep_name = dep.val.as_str();
                    let rng = dependency_bounds(dep);
                    deps_array.push((dep_name, rng));
                }
            }
            index.add_deps(&opam_data.name, pkg_version, &deps_array);
        }
    }
    Ok(index)
}
