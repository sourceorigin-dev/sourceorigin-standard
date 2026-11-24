//! `check-changed`: CI's quick checks, for only what changed since the
//! merge base with a branch, counting uncommitted and untracked files.
//!
//! A repository describes its CI as a [`Ci`] table: each job of
//! `.github/workflows/ci.yml`, the [`Filter`]s of `.github/filters.yml` that
//! start it, and the [`Recipe`]s it runs. [`Ci::run`] runs the recipes of the
//! jobs the changed paths start, narrowed to the crates they reach, and
//! [`Ci::check_workflow`] fails when the table and `ci.yml` disagree.

mod change;
mod filters;
#[cfg(test)]
mod tests;
mod workflow;

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::CommandError;
use crate::config::{Builds, Config, ConfigError};
use crate::metadata::{Metadata, MetadataError};
use crate::workspace::Workspace;
use change::{Outcome, Scope, Step};
use filters::Filters;
use workflow::Workflow;

/// The branch a change merges into, unless the developer names another.
pub const DEFAULT_BASE: &str = "origin/main";

/// A filter of `.github/filters.yml`, as a repository's xtask names it: a
/// closed enum, so a job's trigger can't name a filter that doesn't exist,
/// and loading the file fails when it names a filter the enum doesn't, or
/// misses one it does.
pub trait Filter: Copy + Ord + 'static {
    /// Every filter.
    const ALL: &'static [Self];

    /// Its name in `.github/filters.yml`.
    fn name(self) -> &'static str;
}

/// A repository's CI, as `check-changed` runs it: the jobs of
/// `.github/workflows/ci.yml` that run on a pull request, in the order the
/// workflow lists them.
#[derive(Clone, Copy, Debug)]
pub struct Ci<F: 'static> {
    /// The jobs `check-changed` knows.
    pub jobs: &'static [Job<F>],
    /// The workflow's other jobs, which `check-changed` doesn't run, such as
    /// the `ci` summary job. Every job of `ci.yml` but `changes` is in
    /// `jobs` or here.
    pub ignored: &'static [&'static str],
}

/// A job of `.github/workflows/ci.yml`.
#[derive(Clone, Copy, Debug)]
pub struct Job<F: 'static> {
    /// Its key under `jobs:`.
    pub id: &'static str,
    /// What starts it, as its `if:` reads the `changes` job's outputs.
    pub trigger: Trigger<F>,
    /// The recipes it runs, in order.
    pub recipes: &'static [Recipe],
}

/// What starts a job on a pull request.
#[derive(Clone, Copy, Debug)]
pub enum Trigger<F: 'static> {
    /// Every change: the job's `if:` reads none of the `changes` outputs.
    EveryChange,
    /// A change matching any of these filters.
    AnyOf(&'static [F]),
}

/// A check a job runs.
#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    /// The command that runs it on the whole workspace, such as
    /// `["just", "lint"]`.
    pub command: &'static [&'static str],
    /// Where it runs, and on which crates.
    pub run: Run,
}

/// Where a recipe runs, and on which crates.
#[derive(Clone, Copy, Debug)]
pub enum Run {
    /// Here, on the whole workspace, whatever changed.
    Workspace,
    /// Here, on the crates the change reaches, passed as
    /// `--package=<crate>` arguments; only those with a library when
    /// `libraries_only`, as doc tests need one.
    Crates {
        /// Whether to pass only the crates with a library.
        libraries_only: bool,
    },
    /// Here, on the whole workspace, when the change reaches one of these
    /// crates, as for a check of files generated from them.
    Only(&'static [&'static str]),
    /// Only in CI, for this reason, such as a slow job or a tool a developer
    /// may lack. `check-changed` prints the command.
    InCi(&'static str),
}

/// Why `check-changed` couldn't decide or run the checks.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A file it reads couldn't be read.
    #[error("read {}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// Why it couldn't be read.
        #[source]
        source: std::io::Error,
    },
    /// A file it reads isn't valid YAML.
    #[error("parse {file}")]
    Yaml {
        /// The file, from the repository's root.
        file: &'static str,
        /// Where and why parsing failed.
        #[source]
        source: yaml_rust2::ScanError,
    },
    /// A file it reads isn't shaped as GitHub or paths-filter expect.
    #[error("{file}: expected {expected}")]
    Shape {
        /// The file, from the repository's root.
        file: &'static str,
        /// What it expected to find.
        expected: &'static str,
    },
    /// `.github/filters.yml` names a filter the xtask's [`Filter`] enum
    /// doesn't.
    #[error(
        "{}: unknown filter `{name}`: add it to the xtask's filter enum, and to the jobs it starts",
        filters::PATH
    )]
    UnknownFilter {
        /// The filter's name.
        name: String,
    },
    /// The xtask's [`Filter`] enum names a filter `.github/filters.yml`
    /// doesn't.
    #[error(
        "{} has no filter `{name}`: remove it from the xtask's filter enum, and from the jobs it starts",
        filters::PATH
    )]
    MissingFilter {
        /// The filter's name.
        name: &'static str,
    },
    /// A filter holds something `check-changed` can't match as paths-filter
    /// does, such as a map from change types to globs, or an extglob.
    #[error("{}: filter `{filter}`: unsupported {item}", filters::PATH)]
    Unsupported {
        /// The filter's name.
        filter: String,
        /// What it holds.
        item: String,
    },
    /// A filter's glob is invalid.
    #[error("{}: filter `{filter}`", filters::PATH)]
    Glob {
        /// The filter's name.
        filter: String,
        /// Which glob, and why.
        #[source]
        source: globset::Error,
    },
    /// The `changes` job's paths-filter step reads other filters than
    /// `.github/filters.yml`, such as ones written inline.
    #[error(
        "{}: the `changes` job's paths-filter step must read `filters: {}`",
        workflow::PATH,
        filters::PATH
    )]
    InlineFilters,
    /// The paths-filter step names a `predicate-quantifier` it doesn't
    /// support.
    #[error("{}: unknown predicate-quantifier `{found}`", workflow::PATH)]
    UnknownQuantifier {
        /// The quantifier.
        found: String,
    },
    /// `.github/workflows/ci.yml` and the xtask's [`Ci`] table disagree.
    #[error(
        "{} and the xtask's CI table disagree:{}",
        workflow::PATH,
        Drifts(.0)
    )]
    Drift(Vec<Drift>),
    /// The table or `standard.toml` names a crate that isn't in the
    /// workspace.
    #[error("the xtask's CI table or standard.toml names `{name}`, which is no workspace crate")]
    UnknownCrate {
        /// The crate's name.
        name: String,
    },
    /// `standard.toml`, which says which crates build others, couldn't be
    /// loaded.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// The merge base with the branch couldn't be found, as when it hasn't
    /// been fetched.
    #[error("find the merge base of HEAD and {base}: fetch it, or name another branch")]
    MergeBase {
        /// The branch.
        base: String,
        /// Why git couldn't find it.
        #[source]
        source: CommandError,
    },
    /// `cargo metadata` couldn't describe the workspace.
    #[error(transparent)]
    Metadata(#[from] MetadataError),
    /// A command failed.
    #[error(transparent)]
    Command(#[from] CommandError),
}

/// A way `.github/workflows/ci.yml` and a [`Ci`] table disagree.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Drift {
    /// A job of the table isn't in the workflow.
    #[error("job `{job}` is in the table, not the workflow")]
    Missing {
        /// The job's key.
        job: &'static str,
    },
    /// A job of the workflow is neither in the table nor ignored, so
    /// `check-changed` would never run it.
    #[error("job `{job}` is in the workflow, but neither in the table nor ignored")]
    Unlisted {
        /// The job's key.
        job: String,
    },
    /// The table ignores a job the workflow doesn't have.
    #[error("job `{job}` is ignored, but isn't in the workflow")]
    StaleIgnore {
        /// The job's key.
        job: &'static str,
    },
    /// A job's `run:` scripts call other `just` recipes than the table's
    /// recipes for it.
    #[error(
        "job `{job}` runs {} in the table, but {} in the workflow",
        Recipes(table),
        Recipes(workflow)
    )]
    Recipes {
        /// The job's key.
        job: &'static str,
        /// The `just` recipes the table gives it.
        table: BTreeSet<String>,
        /// The `just` recipes its `run:` scripts call.
        workflow: BTreeSet<String>,
    },
    /// A job's `if:` reads an output the `changes` job doesn't set.
    #[error("job `{job}` reads `needs.changes.outputs.{output}`, which `changes` doesn't set")]
    UnknownOutput {
        /// The job's key.
        job: String,
        /// The output's name.
        output: String,
    },
    /// A job's `if:` reads the outputs of other filters than the table
    /// says start it.
    #[error(
        "job `{job}` starts on {} in the table, but on {} in the workflow",
        Names(table),
        Names(workflow)
    )]
    Trigger {
        /// The job's key.
        job: &'static str,
        /// The filters the table names.
        table: BTreeSet<String>,
        /// The filters the outputs its `if:` reads are built from.
        workflow: BTreeSet<String>,
    },
}

/// `just` recipe names as a sentence lists them.
struct Recipes<'a>(&'a BTreeSet<String>);

impl fmt::Display for Recipes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("no `just` recipe");
        }
        let names: Vec<String> = self.0.iter().map(|name| format!("`just {name}`")).collect();
        f.write_str(&names.join(", "))
    }
}

/// Drifts as an error lists them, one per line.
struct Drifts<'a>(&'a [Drift]);

impl fmt::Display for Drifts<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .try_for_each(|drift| write!(f, "\n  - {drift}"))
    }
}

/// Filter names as a sentence lists them, `every change` when there are
/// none.
struct Names<'a, T>(&'a BTreeSet<T>);

impl<T: fmt::Display> fmt::Display for Names<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return f.write_str("every change");
        }
        for (index, name) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(" or ")?;
            }
            write!(f, "`{name}`")?;
        }
        Ok(())
    }
}

impl<F: Filter> Ci<F> {
    /// Runs the checks the change since the merge base of HEAD and `base`
    /// needs, from `root`, the repository's root, printing each step and a
    /// summary, and returns [`ExitCode::FAILURE`] when a check failed.
    ///
    /// # Errors
    ///
    /// When a file it reads is missing or malformed, the table and the
    /// workflow disagree ([`Error::Drift`]), or a command can't run.
    pub fn run(&self, root: &Path, base: &str) -> Result<ExitCode, Error> {
        let filters = self.load(root)?;
        let workspace = Workspace::new(root);
        let (merge_base, paths) = workspace.changed_paths(base)?;
        if paths.is_empty() {
            println!("Nothing changed since {merge_base}, the merge base with {base}.");
            return Ok(ExitCode::SUCCESS);
        }
        let metadata = Metadata::load(workspace)?;
        let builds = Config::load(root)?.check_changed.builds;
        self.check_crates(&metadata, &builds)?;

        let matched = filters.matching(&paths);
        let names: Vec<_> = matched.iter().map(|filter| filter.name()).collect();
        println!(
            "{} paths changed since {merge_base}, the merge base with {base}, matching: {}.",
            paths.len(),
            if names.is_empty() {
                "no filter".to_owned()
            } else {
                names.join(", ")
            }
        );
        let mut steps = Vec::new();
        for job in self.jobs {
            let scope = job.trigger.fires(&matched).then(|| {
                let covered = paths
                    .iter()
                    .filter(|path| job.trigger.covers(&filters, path));
                Scope::new(&metadata, &builds, covered)
            });
            if let Some(scope) = &scope
                && job.recipes.iter().any(|recipe| recipe.run.scoped())
            {
                println!("{}: {scope}.", job.id);
            }
            for recipe in job.recipes {
                let mut step = Step::plan(job, recipe, scope.as_ref());
                step.run(workspace)?;
                steps.push(step);
            }
        }

        println!("\nSummary:");
        for step in &steps {
            println!("{step}");
        }
        if steps
            .iter()
            .any(|step| matches!(step.outcome, Outcome::LeftToCi(_)))
        {
            println!(
                "\nCI runs the checks left to it. Run one here, with its command above, \
                 when the change risks breaking it."
            );
        }
        Ok(
            if steps.iter().any(|step| step.outcome == Outcome::Failed) {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            },
        )
    }

    /// Checks the table against `ci.yml` and `filters.yml` under `root`:
    /// each job of either is in the other, its `if:` reads the `changes`
    /// outputs of exactly the filters its trigger names, and the filters
    /// file names exactly the [`Filter`]s. A repository calls it from a unit
    /// test; [`Ci::run`] checks it too.
    ///
    /// # Errors
    ///
    /// [`Error::Drift`] listing every disagreement, or a file's load error.
    pub fn check_workflow(&self, root: &Path) -> Result<(), Error> {
        self.load(root).map(drop)
    }

    /// The filters under `root`, matched as the workflow's paths-filter step
    /// matches them, once the table and the workflow agree.
    fn load(&self, root: &Path) -> Result<Filters<F>, Error> {
        let workflow = Workflow::load(root)?;
        let filters = Filters::load(root, workflow.quantifier)?;
        let drifts = self.drifts(&workflow);
        if drifts.is_empty() {
            Ok(filters)
        } else {
            Err(Error::Drift(drifts))
        }
    }

    /// The ways the table and `workflow` disagree.
    fn drifts(&self, workflow: &Workflow) -> Vec<Drift> {
        let mut drifts = Vec::new();
        for job in self.jobs {
            let Some(found) = workflow.jobs.get(job.id) else {
                drifts.push(Drift::Missing { job: job.id });
                continue;
            };
            let mut filters = BTreeSet::new();
            for output in &found.outputs {
                match workflow.outputs.get(output) {
                    Some(built_from) => filters.extend(built_from.iter().cloned()),
                    None => drifts.push(Drift::UnknownOutput {
                        job: job.id.to_owned(),
                        output: output.clone(),
                    }),
                }
            }
            let table = job.trigger.names();
            if table != filters {
                drifts.push(Drift::Trigger {
                    job: job.id,
                    table,
                    workflow: filters,
                });
            }
        }
        for (id, found) in &workflow.jobs {
            let entries: Vec<&Job<F>> = self
                .jobs
                .iter()
                .filter(|job| job.id == id.as_str())
                .collect();
            let Some(first) = entries.first() else {
                if id != workflow::CHANGES && !self.ignored.contains(&id.as_str()) {
                    drifts.push(Drift::Unlisted { job: id.clone() });
                }
                continue;
            };
            let table: BTreeSet<String> = entries
                .iter()
                .flat_map(|job| job.recipes)
                .filter_map(Recipe::just)
                .map(str::to_owned)
                .collect();
            if table != found.recipes {
                drifts.push(Drift::Recipes {
                    job: first.id,
                    table,
                    workflow: found.recipes.clone(),
                });
            }
        }
        drifts.extend(
            self.ignored
                .iter()
                .filter(|job| !workflow.jobs.contains_key(**job))
                .map(|&job| Drift::StaleIgnore { job }),
        );
        drifts
    }

    /// Checks that every crate the table and `builds` name is in the
    /// workspace.
    fn check_crates(&self, metadata: &Metadata, builds: &[Builds]) -> Result<(), Error> {
        let only = self
            .jobs
            .iter()
            .flat_map(|job| job.recipes)
            .flat_map(|recipe| match recipe.run {
                Run::Only(names) => names,
                Run::Workspace | Run::Crates { .. } | Run::InCi(_) => &[],
            })
            .copied();
        let builds = builds
            .iter()
            .flat_map(|builds| [builds.builder.as_str(), builds.built.as_str()]);
        match only.chain(builds).find(|&name| !metadata.has(name)) {
            Some(name) => Err(Error::UnknownCrate {
                name: name.to_owned(),
            }),
            None => Ok(()),
        }
    }
}

impl<F: Filter> Trigger<F> {
    /// Whether a change matching `matched` starts the job.
    fn fires(self, matched: &BTreeSet<F>) -> bool {
        match self {
            Self::EveryChange => true,
            Self::AnyOf(filters) => filters.iter().any(|filter| matched.contains(filter)),
        }
    }

    /// Whether `path` alone would start the job, so its crates are the
    /// job's to check.
    fn covers(self, filters: &Filters<F>, path: &str) -> bool {
        match self {
            Self::EveryChange => true,
            Self::AnyOf(any) => any.iter().any(|&filter| filters.matches(filter, path)),
        }
    }

    /// The names of the filters it names, none for every change.
    fn names(self) -> BTreeSet<String> {
        match self {
            Self::EveryChange => BTreeSet::new(),
            Self::AnyOf(filters) => filters
                .iter()
                .map(|filter| filter.name().to_owned())
                .collect(),
        }
    }
}

impl<F: Filter> fmt::Display for Trigger<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Names(&self.names()).fmt(f)
    }
}

impl Recipe {
    /// The `just` recipe it runs, such as `lint` for `just lint --workspace`,
    /// if it runs one.
    fn just(&self) -> Option<&'static str> {
        match self.command {
            ["just", recipe, ..] => Some(recipe),
            _ => None,
        }
    }
}

impl Run {
    /// Whether it depends on the crates a change reaches.
    fn scoped(self) -> bool {
        matches!(self, Self::Crates { .. } | Self::Only(_))
    }
}
