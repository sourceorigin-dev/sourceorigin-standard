//! A change: the paths it touches, the crates they reach, and the steps
//! that check it.

use std::collections::BTreeSet;
use std::fmt;

use super::metadata::Metadata;
use super::{Builds, Error, Filter, Job, Recipe, Run};
use crate::CommandError;
use crate::workspace::Workspace;

impl Workspace<'_> {
    /// The merge base of HEAD and `base`, and the paths changed since,
    /// from the repository's root: committed, staged, unstaged, and
    /// untracked but not ignored. A renamed file counts under both paths,
    /// as paths-filter counts it.
    pub(super) fn changed_paths(self, base: &str) -> Result<(String, BTreeSet<String>), Error> {
        let merge_base = self
            .output("git", &["merge-base", base, "HEAD"])
            .map_err(|source| Error::MergeBase {
                base: base.to_owned(),
                source,
            })?
            .trim()
            .to_owned();
        // Against the working tree, so it includes uncommitted changes.
        let diff = self.output(
            "git",
            &["diff", "--name-only", "--no-renames", "-z", &merge_base],
        )?;
        let untracked = self.output(
            "git",
            &[
                "ls-files",
                "--others",
                "--exclude-standard",
                "--full-name",
                "-z",
            ],
        )?;
        let paths = diff
            .split('\0')
            .chain(untracked.split('\0'))
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
            .collect();
        Ok((merge_base, paths))
    }
}

/// The crates a change reaches: those it changes, and those depending on
/// them.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Scope {
    /// Every crate, since `path` is in none, such as `Cargo.lock`.
    Workspace {
        path: String,
    },
    Crates(BTreeSet<Crate>),
}

/// A crate a change reaches.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Crate {
    pub(super) name: String,
    /// Whether it has a library, which doc tests need.
    pub(super) library: bool,
}

impl Scope {
    /// The crates `paths` reach through the dependencies of every kind
    /// `metadata` lists, and `builds`.
    pub(super) fn new<'a>(
        metadata: &Metadata,
        builds: &[Builds],
        paths: impl IntoIterator<Item = &'a String>,
    ) -> Self {
        let mut reached = BTreeSet::new();
        for path in paths {
            match metadata.package_of(path) {
                Some(package) => reached.insert(package.name.as_str()),
                None => return Self::Workspace { path: path.clone() },
            };
        }
        // Until no crate depends on one reached but is unreached itself.
        while let Some(dependent) = metadata.packages.iter().find(|package| {
            !reached.contains(package.name.as_str())
                && (package
                    .dependencies
                    .iter()
                    .any(|dependency| reached.contains(dependency.name.as_str()))
                    || builds.iter().any(|builds| {
                        builds.builder == package.name && reached.contains(builds.built)
                    }))
        }) {
            reached.insert(dependent.name.as_str());
        }
        Self::Crates(
            metadata
                .packages
                .iter()
                .filter(|package| reached.contains(package.name.as_str()))
                .map(|package| Crate {
                    name: package.name.clone(),
                    library: package.has_library(),
                })
                .collect(),
        )
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workspace { path } => write!(f, "every crate, as {path} is in none"),
            Self::Crates(crates) if crates.is_empty() => f.write_str("no crate"),
            Self::Crates(crates) => {
                let names: Vec<_> = crates.iter().map(|krate| krate.name.as_str()).collect();
                write!(
                    f,
                    "{}, changed or depending on one that is",
                    names.join(", ")
                )
            }
        }
    }
}

/// A recipe of a job, as `check-changed` plans and runs it.
#[derive(Debug)]
pub(super) struct Step {
    /// The job's key.
    job: &'static str,
    /// The command that runs it, narrowed to the crates the change reaches.
    pub(super) command: Vec<String>,
    pub(super) outcome: Outcome,
}

/// Whether a step ran, and how it went.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    /// The change needs it here.
    ToRun,
    Passed,
    Failed,
    /// The change doesn't need it, for this reason.
    Skipped(String),
    /// The change needs it, but CI runs it, for this reason.
    LeftToCi(&'static str),
}

impl Step {
    /// Plans `recipe` of `job`: skipped when `scope` is `None`, as the
    /// change doesn't start the job, else as its [`Run`] says for a change
    /// reaching `scope`.
    pub(super) fn plan<F: Filter>(job: &Job<F>, recipe: &Recipe, scope: Option<&Scope>) -> Self {
        let mut command: Vec<String> = recipe.command.iter().map(ToString::to_string).collect();
        let outcome = match scope {
            None => Outcome::Skipped(format!("no change matches {}", job.trigger)),
            Some(scope) => recipe.run.outcome(scope, &mut command),
        };
        Self {
            job: job.id,
            command,
            outcome,
        }
    }

    /// Runs the step if it is to run, recording whether it passed.
    pub(super) fn run(&mut self, workspace: Workspace<'_>) -> Result<(), Error> {
        let (Outcome::ToRun, Some((program, args))) = (&self.outcome, self.command.split_first())
        else {
            return Ok(());
        };
        let command = self.command.join(" ");
        println!("\n==> {}: {command}", self.job);
        let status = workspace
            .command(program)
            .args(args)
            .status()
            .map_err(|source| CommandError::Start { command, source })?;
        self.outcome = if status.success() {
            Outcome::Passed
        } else {
            Outcome::Failed
        };
        Ok(())
    }
}

impl Run {
    /// Whether to run the recipe for a change reaching `scope`, narrowing
    /// its `command` to the crates the change reaches if it does.
    fn outcome(self, scope: &Scope, command: &mut Vec<String>) -> Outcome {
        match (self, scope) {
            (Self::InCi(why), _) => Outcome::LeftToCi(why),
            (Self::Workspace, _)
            | (Self::Crates { .. } | Self::Only(_), Scope::Workspace { .. }) => Outcome::ToRun,
            (Self::Crates { libraries_only }, Scope::Crates(crates)) => {
                let packages: Vec<_> = crates
                    .iter()
                    .filter(|krate| krate.library || !libraries_only)
                    .map(|krate| format!("--package={}", krate.name))
                    .collect();
                if packages.is_empty() {
                    return Outcome::Skipped(
                        "no crate the change reaches has a library".to_owned(),
                    );
                }
                command.extend(packages);
                Outcome::ToRun
            }
            (Self::Only(names), Scope::Crates(crates)) => {
                if crates
                    .iter()
                    .any(|krate| names.contains(&krate.name.as_str()))
                {
                    Outcome::ToRun
                } else {
                    Outcome::Skipped(format!("the change reaches none of {}", names.join(", ")))
                }
            }
        }
    }
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (label, why) = match &self.outcome {
            Outcome::ToRun => ("not run", None),
            Outcome::Passed => ("passed", None),
            Outcome::Failed => ("FAILED", None),
            Outcome::Skipped(why) => ("skipped", Some(why.as_str())),
            Outcome::LeftToCi(why) => ("left to CI", Some(*why)),
        };
        write!(
            f,
            "  {label:<10}  {:<18}  {}",
            self.job,
            self.command.join(" ")
        )?;
        if let Some(why) = why {
            write!(f, "  ({why})")?;
        }
        Ok(())
    }
}
