//! The standard's rules, each a check over a repository.
//!
//! [`check`] runs every [`Rule`] and returns its [`Violation`]s, skipping the
//! rules `standard.toml` deviates from. [`describe`] lists the rules with
//! their docs.

mod budget;
mod level;
mod modules;
mod source;
#[cfg(test)]
mod tests;
mod unsafe_modules;
mod workspace_lints;

use std::fmt;
use std::path::{Path, PathBuf};

use crate::config::{self, Config, ConfigError, Stale};
use crate::metadata::{Metadata, MetadataError};
use crate::workspace::Workspace;
use budget::Budget;
use modules::Modules;
use source::{Source, Unparsed};
use workspace_lints::{LintLevel, Manifest};

/// Declares the rules: each variant's doc comment is the doc [`Rule::doc`]
/// returns, its string its stable name, and its method the check it runs on
/// a [`Repository`].
macro_rules! rules {
    ($($(#[doc = $doc:literal])+ $rule:ident = $name:literal => $check:ident,)*) => {
        /// A rule of the standard. `cargo xtask standard rules` prints each
        /// with its doc.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
        pub enum Rule {
            $($(#[doc = $doc])+ #[serde(rename = $name)] $rule,)*
        }

        impl Rule {
            /// Every rule, in the order they run.
            pub const ALL: &'static [Self] = &[$(Self::$rule),*];

            /// Its stable name, as `standard.toml` and violations name it.
            #[must_use]
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$rule => $name,)*
                }
            }

            /// What it enforces: what it reads, and what fails.
            #[must_use]
            pub fn doc(self) -> &'static str {
                match self {
                    $(Self::$rule => concat!($($doc, "\n"),+),)*
                }
            }

            /// Where `repository` breaks it.
            fn findings(self, repository: &Repository) -> Vec<Finding> {
                match self {
                    $(Self::$rule => repository.$check(),)*
                }
            }
        }
    };
}

rules! {
    /// Every workspace crate's manifest has `[lints] workspace = true`;
    /// `[workspace.lints]` sets each lint the standard requires (the `rust`
    /// and `clippy` lints of its lint block) to `warn`, and every other lint
    /// to `warn` or `allow`.
    WorkspaceLints = "workspace-lints" => workspace_lints,
    /// Every Rust file git tracks or would track parses as Rust, so the
    /// rules can read it.
    RustSyntax = "rust-syntax" => rust_syntax,
    /// `#[expect(unsafe_code, …)]` sits only inside the modules
    /// `standard.toml`'s `unsafe-modules` lists, such as `my_crate::ffi` or
    /// `build_script_build` (a target's root module is its crate's name),
    /// and every listed module holds one.
    UnsafeModules = "unsafe-modules" => unsafe_modules,
    /// Each crate holds at most as many `#[expect]` attributes (inner,
    /// outer and through `cfg_attr`) as `budget.toml`'s `[expect]` table
    /// gives it, none when it lists no ceiling; each name there is a
    /// workspace crate.
    Budget = "budget" => budget,
}

/// Why the rules couldn't be checked.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// `standard.toml` couldn't be loaded.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// `cargo metadata` couldn't list the workspace's crates.
    #[error(transparent)]
    Metadata(#[from] MetadataError),
    /// A file a rule reads couldn't be read.
    #[error("read {}", path.display())]
    Read {
        /// The file.
        path: PathBuf,
        /// Why it couldn't be read.
        #[source]
        source: std::io::Error,
    },
    /// git couldn't list the repository's files.
    #[error(transparent)]
    Command(#[from] crate::CommandError),
    /// A manifest isn't valid TOML.
    #[error("{}: {message}", path.display())]
    Toml {
        /// The file, from the repository's root.
        path: PathBuf,
        /// The parser's message, saying where.
        message: String,
    },
}

/// A place a repository breaks a rule. It prints its location, rule and
/// fix, then the two ways to challenge the rule instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    /// The rule it breaks.
    pub rule: Rule,
    /// The file, from the repository's root.
    pub path: PathBuf,
    /// The line, from 1.
    pub line: usize,
    /// What is wrong, and how to fix it.
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rule = self.rule.name();
        let path = self.path.display();
        write!(
            f,
            "{path}:{}: [{rule}] {}\n  If the rule is wrong here, ask the user, then add \
             `{rule} = {{ reason = \"<why>\", paths = [\"{path}\"] }}` under `[deviations]` \
             in {} (a bare reason applies it to the whole repository), or change the rule in \
             sourceorigin-standard for all.",
            self.line,
            self.message,
            config::PATH
        )
    }
}

/// A place a rule's check finds the repository breaks it.
#[derive(Debug)]
struct Finding {
    path: PathBuf,
    line: usize,
    message: String,
}

impl Repository {
    /// Each Rust file that doesn't parse.
    fn rust_syntax(&self) -> Vec<Finding> {
        self.unparsed
            .iter()
            .map(|unparsed| Finding {
                path: unparsed.path.clone(),
                line: unparsed.line,
                message: format!(
                    "{}: fix it, or deviate by path for a fixture that mustn't parse",
                    unparsed.message
                ),
            })
            .collect()
    }
}

impl Stale {
    /// The violation of `rule` that this stale entry of its deviation,
    /// given for `reason`, is.
    fn violation(self, rule: Rule, reason: &str) -> Violation {
        let name = rule.name();
        let (line, message) = match self {
            Self::Whole(line) => (
                line,
                format!("the repository follows `{name}`: remove its deviation (\"{reason}\")"),
            ),
            Self::Path(listed) | Self::Crate(listed) => (
                listed.line,
                format!(
                    "`{}` in the deviation from `{name}` (\"{reason}\") excuses no \
                     violation: remove it",
                    listed.value
                ),
            ),
        };
        Violation {
            rule,
            path: PathBuf::from(config::PATH),
            line,
            message,
        }
    }
}

impl Finding {
    fn of(self, rule: Rule) -> Violation {
        Violation {
            rule,
            path: self.path,
            line: self.line,
            message: self.message,
        }
    }
}

/// The repository the rules check, loaded once and shared by every rule.
struct Repository {
    config: Config,
    /// The workspace's crates.
    metadata: Metadata,
    /// The manifests of the workspace's crates.
    manifests: Vec<Manifest>,
    /// The lints `[workspace.lints]` sets.
    lint_levels: Vec<LintLevel>,
    /// Every Rust file git tracks or would track that parses, by path.
    sources: Vec<Source>,
    /// Every one that doesn't.
    unparsed: Vec<Unparsed>,
    /// Each Rust file's module paths.
    modules: Modules,
    /// Each crate's ceilings of exceptions.
    budget: Budget,
}

impl Repository {
    /// Loads everything the rules read, so the rules themselves can't
    /// fail.
    fn load(root: &Path) -> Result<Self, Error> {
        let metadata = Metadata::load(Workspace::new(root))?;
        let (sources, unparsed) = Source::load_all(root)?;
        Ok(Self {
            config: Config::load(root)?,
            manifests: Manifest::load_all(&metadata)?,
            lint_levels: LintLevel::load_all(&metadata.workspace_root)?,
            modules: Modules::of(&metadata, &sources),
            budget: Budget::load(root)?,
            metadata,
            sources,
            unparsed,
        })
    }

    /// The violations of every rule, by file and line. A rule
    /// `standard.toml` deviates from yields none, unless the repository
    /// follows it anyway: then the deviation is stale, and that is the
    /// violation.
    fn violations(&self) -> Vec<Violation> {
        let mut violations = Vec::new();
        for &rule in Rule::ALL {
            let findings = rule.findings(self);
            let Some(deviation) = self.config.deviation(rule) else {
                violations.extend(findings.into_iter().map(|finding| finding.of(rule)));
                continue;
            };
            let mut excused = Vec::new();
            for finding in findings {
                let krate = self.crate_of(&finding.path);
                if deviation.excuses(&finding.path, krate) {
                    excused.push((finding.path, krate));
                } else {
                    violations.push(finding.of(rule));
                }
            }
            let excused: Vec<(&Path, Option<&str>)> = excused
                .iter()
                .map(|(path, krate)| (path.as_path(), *krate))
                .collect();
            violations.extend(
                deviation
                    .stale(&excused)
                    .into_iter()
                    .map(|stale| stale.violation(rule, &deviation.reason)),
            );
        }
        violations.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        violations
    }

    /// The name of the crate owning `path`, from the repository's root.
    fn crate_of(&self, path: &Path) -> Option<&str> {
        self.metadata
            .package_of(&path.to_string_lossy())
            .map(|package| package.name.as_str())
    }
}

/// Checks the repository at `root` against every rule, except those its
/// `standard.toml` deviates from, and returns the violations by file and
/// line. A repository's `cargo xtask standard` calls it, and fails when it
/// returns any.
///
/// # Errors
///
/// When `standard.toml` or a file a rule reads can't be loaded.
pub fn check(root: &Path) -> Result<Vec<Violation>, Error> {
    Ok(Repository::load(root)?.violations())
}

/// Every rule with its doc, as a Markdown list: what
/// `cargo xtask standard rules` prints.
#[must_use]
pub fn describe() -> String {
    Rule::ALL
        .iter()
        .map(|rule| {
            let doc: Vec<&str> = rule.doc().lines().map(str::trim).collect();
            format!("- `{}`: {}\n", rule.name(), doc.join(" "))
        })
        .collect::<Vec<_>>()
        .concat()
}
