//! `check-changed`: the checks CI's quick jobs run, for only what changed
//! since the merge base with a branch, counting uncommitted and untracked
//! files.
//!
//! A repository describes its CI as a [`Ci`] table: each job of
//! `.github/workflows/ci.yml`, the [`Filter`]s of `.github/filters.yml` that
//! start it, and the [`Recipe`]s it runs. [`Ci::run`] matches the changed
//! paths against the filters as CI's `changes` job does with
//! dorny/paths-filter, runs the recipes of each job they start, narrowing
//! those that take crates to the changed crates and the crates depending on
//! them, names the ones it leaves to CI, and prints a summary.
//! [`Ci::check_workflow`] fails when the table and `ci.yml` disagree.

mod change;
mod filters;
mod metadata;
#[cfg(test)]
mod tests;
mod workflow;

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::CommandError;
use crate::workspace::Workspace;
use change::{Outcome, Scope, Step};
use filters::Filters;
use metadata::Metadata;
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
    /// The jobs `check-changed` knows. Every job whose `if:` reads the
    /// `changes` job's outputs must be one of them.
    pub jobs: &'static [Job<F>],
    /// Crates that build another without depending on it, which
    /// `cargo metadata` doesn't show, such as a build script compiling a
    /// crate it embeds.
    pub builds: &'static [Builds],
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

/// A crate that builds another without depending on it.
#[derive(Clone, Copy, Debug)]
pub struct Builds {
    /// The crate that builds `built`, so a change reaching `built` reaches
    /// it too.
    pub builder: &'static str,
    /// The crate it builds.
    pub built: &'static str,
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
    /// The table names a crate that isn't in the workspace.
    #[error("the xtask's CI table names `{name}`, which is no workspace crate")]
    UnknownCrate {
        /// The crate's name.
        name: &'static str,
    },
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
    /// `cargo metadata`'s output couldn't be parsed.
    #[error("parse `cargo metadata`")]
    Metadata(#[source] serde_json::Error),
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
    /// A job of the workflow reads the `changes` outputs but isn't in the
    /// table, so `check-changed` would never run it.
    #[error("job `{job}` reads the `changes` outputs, but isn't in the table")]
    Unlisted {
        /// The job's key.
        job: String,
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
    /// needs, from `root`, the repository's root, and prints each step and
    /// a summary. It fails with [`ExitCode::FAILURE`] when a check failed,
    /// and with an [`Error`] when it couldn't decide which to run, as when
    /// the table and the workflow disagree.
    ///
    /// # Errors
    ///
    /// When a file it reads is missing or malformed, the table and the
    /// workflow disagree ([`Error::Drift`]), git can't find the merge base,
    /// or a command can't start.
    pub fn run(&self, root: &Path, base: &str) -> Result<ExitCode, Error> {
        let filters = self.load(root)?;
        let workspace = Workspace::new(root);
        let (merge_base, paths) = workspace.changed_paths(base)?;
        if paths.is_empty() {
            println!("Nothing changed since {merge_base}, the merge base with {base}.");
            return Ok(ExitCode::SUCCESS);
        }
        let metadata = Metadata::load(workspace)?;
        self.check_crates(&metadata)?;

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
                Scope::new(&metadata, self.builds, covered)
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

    /// Checks the table against `.github/workflows/ci.yml` and
    /// `.github/filters.yml` under `root`: every job of the table is in the
    /// workflow, whose `if:` reads the `changes` outputs of exactly the
    /// filters its trigger names; every job reading those outputs is in the
    /// table; and the filters file names exactly the [`Filter`]s. A
    /// repository calls it from a unit test, so CI fails until the two
    /// change together. [`Ci::run`] checks it too.
    ///
    /// # Errors
    ///
    /// [`Error::Drift`] listing every disagreement, or the error that kept
    /// either file from loading.
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
            let Some(outputs) = workflow.jobs.get(job.id) else {
                drifts.push(Drift::Missing { job: job.id });
                continue;
            };
            let mut filters = BTreeSet::new();
            for output in outputs {
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
        drifts.extend(
            workflow
                .jobs
                .iter()
                .filter(|(id, outputs)| {
                    !outputs.is_empty() && !self.jobs.iter().any(|job| job.id == id.as_str())
                })
                .map(|(id, _)| Drift::Unlisted { job: id.clone() }),
        );
        drifts
    }

    /// Checks that every crate the table names is in the workspace.
    fn check_crates(&self, metadata: &Metadata) -> Result<(), Error> {
        let only = self
            .jobs
            .iter()
            .flat_map(|job| job.recipes)
            .flat_map(|recipe| match recipe.run {
                Run::Only(names) => names,
                Run::Workspace | Run::Crates { .. } | Run::InCi(_) => &[],
            });
        let builds = self
            .builds
            .iter()
            .flat_map(|builds| [builds.builder, builds.built]);
        match only
            .copied()
            .chain(builds)
            .find(|&name| !metadata.has(name))
        {
            Some(name) => Err(Error::UnknownCrate { name }),
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

impl Run {
    /// Whether it depends on the crates a change reaches.
    fn scoped(self) -> bool {
        matches!(self, Self::Crates { .. } | Self::Only(_))
    }
}
