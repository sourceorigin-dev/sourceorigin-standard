//! `.github/workflows/ci.yml`, reduced to how its jobs read the `changes`
//! job's path filters.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use yaml_rust2::{Yaml, YamlLoader};

use super::Error;
use super::filters::{self, Quantifier};
use super::modules::Modules;

/// Where the workflow lives, from the repository's root.
pub(super) const PATH: &str = ".github/workflows/ci.yml";

/// The job that runs paths-filter, whose outputs the other jobs read.
pub(super) const CHANGES: &str = "changes";

/// The action that matches the changed paths against the filters.
const PATHS_FILTER: &str = "dorny/paths-filter@";

/// What the workflow's jobs read of the `changes` job.
#[derive(Debug)]
pub(super) struct Workflow {
    /// How its paths-filter step combines a filter's patterns.
    pub(super) quantifier: Quantifier,
    /// Each output of the `changes` job, and the filters its expression
    /// reads from the paths-filter step.
    pub(super) outputs: BTreeMap<String, BTreeSet<String>>,
    /// Each job, by its key.
    pub(super) jobs: BTreeMap<String, WorkflowJob>,
    /// The justfile's modules, through which its jobs and the table name
    /// their recipes.
    pub(super) modules: Modules,
}

/// What check-changed reads of one job.
#[derive(Debug, Default)]
pub(super) struct WorkflowJob {
    /// The `changes` outputs its `if:` reads.
    pub(super) outputs: BTreeSet<String>,
    /// The `just` recipes its `run:` scripts call, matrix entries included,
    /// each as its path through the justfile's modules, such as `web test`.
    pub(super) recipes: BTreeSet<String>,
}

impl Workflow {
    /// Loads the workflow under `root`, and the modules of its justfile.
    pub(super) fn load(root: &Path) -> Result<Self, Error> {
        let path = root.join(PATH);
        let yaml = std::fs::read_to_string(&path).map_err(|source| Error::Read { path, source })?;
        Self::parse(&yaml, Modules::load(root)?)
    }

    /// Parses a workflow whose `changes` job runs paths-filter on
    /// `.github/filters.yml`, reading its recipes through `modules`.
    pub(super) fn parse(yaml: &str, modules: Modules) -> Result<Self, Error> {
        let documents =
            YamlLoader::load_from_str(yaml).map_err(|source| Error::Yaml { file: PATH, source })?;
        let jobs = documents
            .first()
            .and_then(|document| document["jobs"].as_hash())
            .ok_or(Error::Shape {
                file: PATH,
                expected: "a map of jobs",
            })?;
        let changes = &documents[0]["jobs"][CHANGES];
        let step = changes["steps"]
            .as_vec()
            .into_iter()
            .flatten()
            .find(|step| {
                step["uses"]
                    .as_str()
                    .is_some_and(|uses| uses.starts_with(PATHS_FILTER))
            })
            .ok_or(Error::Shape {
                file: PATH,
                expected: "a `changes` job with a dorny/paths-filter step",
            })?;
        let id = step["id"].as_str().ok_or(Error::Shape {
            file: PATH,
            expected: "an `id` on the paths-filter step, which the `changes` outputs read",
        })?;
        if step["with"]["filters"].as_str() != Some(filters::PATH) {
            return Err(Error::InlineFilters);
        }
        let quantifier = match step["with"]["predicate-quantifier"].as_str() {
            Some(input) => Quantifier::parse(input)?,
            None => Quantifier::default(),
        };
        let filter_prefix = format!("steps.{id}.outputs.");
        let outputs = changes["outputs"]
            .as_hash()
            .into_iter()
            .flatten()
            .filter_map(|(name, expression)| {
                Some((
                    name.as_str()?.to_owned(),
                    Self::references(expression, &filter_prefix),
                ))
            })
            .collect();
        let jobs = jobs
            .iter()
            .filter_map(|(id, job)| {
                let mut recipes = BTreeSet::new();
                Self::recipes(job, &modules, &mut recipes);
                Some((
                    id.as_str()?.to_owned(),
                    WorkflowJob {
                        outputs: Self::references(&job["if"], "needs.changes.outputs."),
                        recipes,
                    },
                ))
            })
            .collect();
        Ok(Self {
            quantifier,
            outputs,
            jobs,
            modules,
        })
    }

    /// Adds the `just` recipes the `run:` scripts anywhere in `node` call,
    /// through `modules`, such as `lint` in `just lint`, `docs` in
    /// `nix develop --command just docs` or `web test` in `just web test`.
    #[cfg_attr(
        dylint_lib = "standard_lints",
        expect(
            string_literal_logic,
            reason = "reads ci.yml's `run` steps and their `just` commands, which the YAML holds as text"
        )
    )]
    fn recipes(node: &Yaml, modules: &Modules, recipes: &mut BTreeSet<String>) {
        match node {
            Yaml::Hash(entries) => {
                for (key, value) in entries {
                    match (key.as_str(), value.as_str()) {
                        (Some("run"), Some(script)) => {
                            let words: Vec<&str> = script.split_whitespace().collect();
                            recipes.extend(
                                words
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, word)| **word == "just")
                                    .filter_map(|(at, _)| modules.path(&words[at + 1..])),
                            );
                        }
                        _ => Self::recipes(value, modules, recipes),
                    }
                }
            }
            Yaml::Array(items) => {
                for item in items {
                    Self::recipes(item, modules, recipes);
                }
            }
            _ => {}
        }
    }

    /// The names an expression reads after `prefix`, such as `rust` in
    /// `needs.changes.outputs.rust == 'true'`.
    fn references(expression: &Yaml, prefix: &str) -> BTreeSet<String> {
        let expression = expression.as_str().unwrap_or_default();
        expression
            .match_indices(prefix)
            .map(|(start, _)| {
                expression[start + prefix.len()..]
                    .chars()
                    .take_while(|&c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    .collect::<String>()
            })
            .filter(|name| !name.is_empty())
            .collect()
    }
}
