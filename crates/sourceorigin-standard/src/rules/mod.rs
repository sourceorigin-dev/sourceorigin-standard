//! The standard's rules, each a check over a repository.
//!
//! [`check`] runs every [`Rule`] and returns its [`Violation`]s, skipping the
//! rules `standard.toml` deviates from. [`describe`] lists the rules with
//! their docs.

mod agent_files;
mod agents_md;
mod anyhow_edges;
mod budget;
mod cargo_locked;
mod clippy_config;
mod comments;
mod config_comments;
mod deny_config;
mod fix;
mod generated_lints;
mod justfile;
mod layout;
mod level;
mod manifest;
mod modules;
mod nextest_config;
mod recipes;
mod recipes_only;
mod report;
mod rustfmt_config;
mod settings;
mod source;
mod t3;
#[cfg(test)]
mod tests;
mod text_file;
mod toml_file;
mod toolchain;
mod unsafe_modules;
mod workflows;
mod workspace_dependencies;
mod workspace_lints;
mod workspace_manifest;
mod yaml;

use std::fmt;
use std::path::{Path, PathBuf};

use crate::config::{self, Config, ConfigError, Stale};
use crate::metadata::{Metadata, MetadataError};
use crate::workspace::Workspace;
use agent_files::AgentFiles;
use budget::Budget;
use config_comments::Commented;
use deny_config::DenyConfig;
use fix::Checked;
pub use fix::Fix;
use justfile::Justfile;
use layout::CargoConfig;
use manifest::{Manifest, RootManifest};
use modules::Modules;
use nextest_config::NextestConfig;
pub use report::{Report, RuleReport};
use settings::Settings;
use source::{Source, Unparsed};
use t3::T3;
use text_file::TextFile;
use toml_file::TomlFile;
use toolchain::ToolchainFile;
use workflows::Workflow;
use workspace_lints::LintLevel;

/// Declares the rules: each variant's doc comment is the doc [`Rule::doc`]
/// returns, its string its stable name, and its method the check it runs on
/// a [`Repository`].
macro_rules! rules {
    ($($(#[doc = $doc:literal])+ $rule:ident = $name:literal => $check:ident,)*) => {
        /// A rule of the standard. `cargo xtask standard rules` prints each
        /// with its doc.
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize, serde::Serialize,
        )]
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
            fn check(self, repository: &Repository) -> Checked {
                match self {
                    $(Self::$rule => repository.$check().into(),)*
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
    /// Every Rust file git tracks or would track parses as Rust.
    RustSyntax = "rust-syntax" => rust_syntax,
    /// `#[expect(unsafe_code, …)]` sits only inside the modules
    /// `standard.toml`'s `unsafe-modules` lists, such as `my_crate::ffi` or
    /// `build_script_build` (a target's root module is its crate's name),
    /// and every listed module holds one.
    UnsafeModules = "unsafe-modules" => unsafe_modules,
    /// Each crate holds at most as many `#[expect]` attributes (inner,
    /// outer and through `cfg_attr`) as `budget.toml`'s `[expect]` table
    /// gives it, and at most as many lines starting `// INVARIANT:` as its
    /// `[invariant]` table does; none for a crate a table doesn't list. Each
    /// name there is a workspace crate.
    Budget = "budget" => budget,
    /// Rust files `.gitattributes` marks `linguist-generated` contain no lint
    /// attribute (`allow`, `expect`, `warn`, `deny` or `forbid`, inner or
    /// outer): their exceptions sit on their `mod` declaration.
    GeneratedLints = "generated-lints" => generated_lints,
    /// Rust files that aren't generated hold only doc comments (`///`,
    /// `//!`), `// SAFETY:` comments and `// INVARIANT:` comments; any other
    /// `//` or `/* */` comment fails.
    RustComments = "rust-comments" => rust_comments,
    /// The prose of each item's or module's docs, in a file that isn't
    /// generated, spans at most `standard.toml`'s `[comments] doc-lines`
    /// lines (10 by default). Code blocks and a crate root's `//!` docs
    /// don't count.
    DocLength = "doc-length" => doc_length,
    /// In every `Cargo.toml`, justfile (and `*.just`) and `.github` YAML file,
    /// each block of `#` comment lines spans at most `standard.toml`'s
    /// `[comments] block-lines` (6 by default), and the file holds at most
    /// `[comments] file-lines` comment lines (40 by default).
    ConfigComments = "config-comments" => config_comments,
    /// The root manifest's `[workspace]` sets `resolver = "3"`, its
    /// `[workspace.package]` sets `edition = "2024"`, and every workspace
    /// crate inherits it with `edition.workspace = true`.
    WorkspaceManifest = "workspace-manifest" => workspace_manifest,
    /// Every workspace crate lives under `crates/`, `apps/` or `tools/`, the
    /// `xtask` crate in `tools/xtask`, and `.cargo/config.toml` aliases
    /// `cargo xtask` to `run --locked --package xtask --`.
    Layout = "layout" => layout,
    /// Every `cargo build`, `check`, `clippy`, `doc`, `llvm-cov`,
    /// `nextest`, `publish`, `run` and `test` in a justfile (and `*.just`),
    /// `.github` YAML file or `t3.json` passes `--locked` (or `--frozen`).
    CargoLocked = "cargo-locked" => cargo_locked,
    /// `AGENTS.md` sits at the root, and no `CLAUDE.md` exists. Each skill
    /// is `.agents/skills/<name>/SKILL.md`, its frontmatter's `name` is
    /// `<name>`, and `.claude/skills` is a symlink to `../.agents/skills`.
    AgentFiles = "agent-files" => agent_files,
    /// AGENTS.md names every workspace crate, by its name in backticks or
    /// its directory, and holds at most 150 lines.
    AgentsMd = "agents-md" => agents_md,
    /// `rust-toolchain.toml` pins `channel = "stable"` and installs at
    /// least the `rustfmt` and `clippy` components.
    Toolchain = "toolchain" => toolchain,
    /// `rustfmt.toml` sets `edition = "2024"`, `newline_style = "Unix"` and
    /// `use_field_init_shorthand = true`.
    RustfmtConfig = "rustfmt-config" => rustfmt_config,
    /// `clippy.toml` sets `allow-expect-in-tests`, `allow-panic-in-tests`
    /// and `allow-dbg-in-tests` to `true`, and never
    /// `allow-unwrap-in-tests`.
    ClippyConfig = "clippy-config" => clippy_config,
    /// `.config/nextest.toml`'s default profile sets a `slow-timeout`, every
    /// `slow-timeout` in it has a `terminate-after`, and no profile or
    /// override sets `retries`.
    NextestConfig = "nextest-config" => nextest_config,
    /// Every dependency of a workspace crate, of every kind and target, is
    /// inherited with `workspace = true` from the root's
    /// `[workspace.dependencies]`, where each `path` dependency also has a
    /// `version`.
    WorkspaceDependencies = "workspace-dependencies" => workspace_dependencies,
    /// `deny.toml` sets `unknown-registry` and `unknown-git` under
    /// `[sources]`, and `wildcards` under `[bans]`, to `"deny"`, allows at
    /// least one license, and gives every ignored advisory a `reason`:
    /// `{ id = "…", reason = "…" }`.
    DenyConfig = "deny-config" => deny_config,
    /// No library depends on `anyhow` (dev-dependencies aside), unless
    /// `standard.toml`'s `anyhow-crates` lists it as a binary's edge, such
    /// as a crate holding a CLI's commands. Each listed name is a workspace
    /// crate.
    AnyhowEdges = "anyhow-edges" => anyhow_edges,
    /// The justfile has the recipes `validate`, `fmt`, `fmt-check`, `lint`,
    /// `standard`, `test`, `doc`, `doc-test`, `audit`, `unused` and
    /// `check-changed`. `just` alone runs `validate` or `check-changed`;
    /// `validate` runs `fmt` first, then at least `lint`, `standard`, `test`,
    /// `doc`, `doc-test`, `audit` and `unused`, and never `fmt-check`; `test`
    /// takes `*ARGS`, defaulting to `--workspace`.
    Recipes = "recipes" => recipes,
    /// Every command of `t3.json` runs `just`, and no line of a workflow's
    /// or composite action's `run:` starts with `cargo` unless
    /// `standard.toml`'s `cargo-in-ci` lists it; each listed line is one a
    /// `run:` holds.
    RecipesOnly = "recipes-only" => recipes_only,
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
    /// `--fix` couldn't write a file.
    #[error("write {}", path.display())]
    Write {
        /// The file.
        path: PathBuf,
        /// Why it couldn't be written.
        #[source]
        source: std::io::Error,
    },
    /// `just --dump` printed what isn't a justfile's description.
    #[error("parse `just --dump --dump-format json`")]
    Justfile(#[source] serde_json::Error),
    /// A workflow isn't valid YAML.
    #[error("parse {}", path.display())]
    Yaml {
        /// The file, from the repository's root.
        path: PathBuf,
        /// Where the parser stopped, and why.
        #[source]
        source: yaml_rust2::ScanError,
    },
    /// A JSON file, such as `t3.json`, isn't valid or has the wrong shape.
    #[error("parse {}", path.display())]
    Json {
        /// The file, from the repository's root.
        path: PathBuf,
        /// Why it couldn't be parsed.
        #[source]
        source: serde_json::Error,
    },
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
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
    /// The root manifest.
    root_manifest: TomlFile<RootManifest>,
    /// `rust-toolchain.toml`.
    toolchain: Option<TomlFile<ToolchainFile>>,
    /// `rustfmt.toml`, empty when missing.
    rustfmt: TomlFile<Settings>,
    /// `clippy.toml`, empty when missing.
    clippy: TomlFile<Settings>,
    /// `.cargo/config.toml`.
    cargo_config: Option<TomlFile<CargoConfig>>,
    /// The manifests of the workspace's crates.
    manifests: Vec<TomlFile<Manifest>>,
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
    /// `AGENTS.md`, any `CLAUDE.md`, and the skills.
    agent_files: AgentFiles,
    /// The justfiles, workflows and `t3.json`, whose commands the rules
    /// read.
    scripts: Vec<TextFile>,
    /// The manifests, justfiles and workflows, with their comments.
    commented: Vec<Commented>,
    /// `.config/nextest.toml`, empty when missing.
    nextest: TomlFile<NextestConfig>,
    /// `deny.toml`.
    deny: Option<TomlFile<DenyConfig>>,
    /// The justfile, as `just` describes it.
    justfile: Option<Justfile>,
    /// `t3.json`.
    t3: Option<T3>,
    /// The workflows and composite actions.
    workflows: Vec<Workflow>,
}

impl Repository {
    /// Loads everything the rules read, so the rules themselves can't
    /// fail.
    fn load(root: &Path) -> Result<Self, Error> {
        let metadata = Metadata::load(Workspace::new(root))?;
        let (sources, unparsed) = Source::load_all(root)?;
        let root_manifest = TomlFile::<RootManifest>::load_root(root)?;
        Ok(Self {
            config: Config::load(root)?,
            manifests: TomlFile::load_all(&metadata)?,
            lint_levels: LintLevel::all(&root_manifest),
            root_manifest,
            cargo_config: TomlFile::load(root, layout::CARGO_CONFIG)?,
            toolchain: TomlFile::load(root, toolchain::PATH)?,
            rustfmt: TomlFile::load_or_empty(root, rustfmt_config::PATH)?,
            clippy: TomlFile::load_or_empty(root, clippy_config::PATH)?,
            modules: Modules::of(&metadata, &sources),
            budget: Budget::load(root)?,
            commented: Commented::load_all(root)?,
            scripts: TextFile::load_all(root, &cargo_locked::FILES)?,
            agent_files: AgentFiles::load(root)?,
            nextest: TomlFile::load_or_empty(root, nextest_config::PATH)?,
            deny: TomlFile::load(root, deny_config::PATH)?,
            justfile: Justfile::load(root)?,
            t3: T3::load(root)?,
            workflows: Workflow::load_all(root)?,
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
            let (kept, stale) = self.deviate(rule, rule.check(self).findings);
            violations.extend(kept.into_iter().map(|finding| finding.of(rule)));
            if let Some(deviation) = self.config.deviation(rule) {
                violations.extend(
                    stale
                        .into_iter()
                        .map(|stale| stale.violation(rule, &deviation.reason)),
                );
            }
        }
        violations.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
        violations
    }

    /// The fixes of every rule the repository doesn't deviate from, and the
    /// removal of each deviation, or entry of one, that excuses nothing.
    fn fixes(&self) -> Vec<Fix> {
        let mut fixes = Vec::new();
        for &rule in Rule::ALL {
            let checked = rule.check(self);
            let Some(deviation) = self.config.deviation(rule) else {
                fixes.extend(checked.fixes);
                continue;
            };
            let (_, stale) = self.deviate(rule, checked.findings);
            if !stale.is_empty() {
                fixes.push(Fix::stale(rule, deviation, &stale));
            }
        }
        fixes
    }

    /// The findings of `rule` its deviation doesn't excuse, and the entries
    /// of the deviation that excuse none of them.
    fn deviate(&self, rule: Rule, findings: Vec<Finding>) -> (Vec<Finding>, Vec<Stale>) {
        let Some(deviation) = self.config.deviation(rule) else {
            return (findings, Vec::new());
        };
        let (excused, kept): (Vec<Finding>, Vec<Finding>) = findings
            .into_iter()
            .partition(|finding| deviation.excuses(&finding.path, self.crate_of(&finding.path)));
        let excused: Vec<(&Path, Option<&str>)> = excused
            .iter()
            .map(|finding| (finding.path.as_path(), self.crate_of(&finding.path)))
            .collect();
        (kept, deviation.stale(&excused))
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

/// Makes every mechanical fix the rules know in the repository at `root`,
/// such as adding `[lints] workspace = true` or lowering a ceiling of
/// `budget.toml` to what the crate holds, and returns them. Each is an edit
/// to a TOML document, which keeps the file's layout and comments, or to a
/// line of another file, such as a justfile. What needs judgement stays a
/// violation for [`check`] to report.
///
/// # Errors
///
/// When a file a rule reads can't be loaded, or a fixed file written.
pub fn fix(root: &Path) -> Result<Vec<Fix>, Error> {
    let mut fixes = Repository::load(root)?.fixes();
    Fix::apply_all(root, &mut fixes)?;
    Ok(fixes)
}

/// The report of the repository at `root`: each rule's violations and
/// deviation, and its `#[expect]` attributes by lint and `// INVARIANT:`
/// comments. `cargo xtask standard report` prints it as JSON.
///
/// # Errors
///
/// When `standard.toml` or a file a rule reads can't be loaded.
pub fn report(root: &Path) -> Result<Report, Error> {
    Ok(Repository::load(root)?.report())
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
