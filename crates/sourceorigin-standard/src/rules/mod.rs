//! The standard's rules, each a check over a repository.
//!
//! [`check`] runs every [`Rule`] and returns its [`Violation`]s, skipping the
//! rules `standard.toml` deviates from. [`describe`] lists the rules with
//! their docs, and [`sync_skills`] writes the crate's agent skills into a
//! repository.

#![cfg_attr(
    dylint_lib = "standard_lints",
    expect(
        string_literal_logic,
        reason = "a linter compares source text with names"
    )
)]

mod actions_pinned;
mod agent_files;
mod agents_md;
mod anyhow_edges;
mod budget;
mod cache_saves;
mod cargo_locked;
mod childless_mod_rs;
mod ci_shape;
mod clippy_config;
mod comments;
mod config_comments;
mod config_syntax;
mod deny_config;
mod docs_images;
mod docs_summary;
mod duplicate_items;
mod fix;
mod generated_lints;
mod inline_limits;
mod inline_tests;
mod job_timeouts;
mod justfile;
mod layout;
mod level;
mod manifest;
mod markdown;
mod modules;
mod nextest_config;
mod no_json_macro;
mod pr_template;
mod recipes;
mod recipes_only;
mod report;
mod route_literals;
mod rustfmt_config;
mod scheduled_audit;
mod settings;
mod skills;
mod source;
mod t3;
mod t3_setup;
mod test_code;
#[cfg(test)]
mod tests;
mod text_file;
mod toml_file;
mod toolchain;
mod tracing_fields;
mod unsafe_modules;
mod walk;
mod wording;
mod workflow_permissions;
mod workflows;
mod workspace_dependencies;
mod workspace_lints;
mod workspace_manifest;
mod yaml;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::config::{self, Config, ConfigError, Stale};
use crate::metadata::{Metadata, MetadataError};
use crate::workspace::{CommandError, Workspace};
use agent_files::AgentFiles;
use budget::{Budget, OtherWorkspace};
use config_comments::Commented;
use config_syntax::Unreadable;
use deny_config::DenyConfig;
use docs_summary::Book;
use fix::Checked;
pub use fix::Fix;
use justfile::Justfile;
use layout::CargoConfig;
use manifest::{Manifest, RootManifest};
use modules::Modules;
use nextest_config::NextestConfig;
pub use report::{Report, RuleReport};
use settings::Settings;
use skills::Synced;
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
    /// The crates' manifests, `budget.toml`, `deny.toml`, `rustfmt.toml`,
    /// `clippy.toml`, `rust-toolchain.toml`, `.config/nextest.toml`,
    /// `.cargo/config.toml`, every `.github` YAML file and `t3.json` parse,
    /// and `just --dump` reads the justfile. When `cargo metadata` can't
    /// read the workspace, the file cargo names, or else the root manifest,
    /// is the only violation, with cargo's whole message.
    ConfigSyntax = "config-syntax" => config_syntax,
    /// `#[expect(unsafe_code, …)]` sits only inside the modules
    /// `standard.toml`'s `unsafe-modules` lists, such as `my_crate::ffi` or
    /// `build_script_build` (a target's root module is its crate's name),
    /// and every listed module holds one.
    UnsafeModules = "unsafe-modules" => unsafe_modules,
    /// Each crate's `#[expect]` lints (each lint an `#[expect]` names,
    /// inner, outer, through `cfg_attr` or in a `macro_rules!` body) number
    /// exactly what `budget.toml`'s `[expect]` table gives it, and its lines
    /// starting `// INVARIANT:` exactly what `[invariant]` does, in the files
    /// its targets reach; each manifest's lints set to `allow` number what
    /// `[allow]` gives its path. Zero for one a table doesn't list. The
    /// crates of other workspaces in the repository count too, but not a
    /// Dylint library's UI cases, which are test fixtures.
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
    /// `nextest`, `publish`, `run` and `test` run by a justfile's recipe
    /// (and a `*.just` file's), a workflow's or composite action's `run:`,
    /// or a `t3.json` command passes `--locked` (or `--frozen`). Cargo runs
    /// only as a command word, after any `NAME=value`, `env`, `exec`, `time`
    /// or `nice -n N`; quoted text and a heredoc's body are no command, and
    /// a folded `run: >` block is one.
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
    /// A recipe `t3.json` runs on worktree creation that runs Cargo, itself
    /// or through the recipes it runs, sets `CARGO_BUILD_JOBS` to a
    /// default, half the cores, that a set one overrides, and runs the build
    /// with `nice -n 10`, both in its own body.
    T3Setup = "t3-setup" => t3_setup,
    /// `.github/workflows/ci.yml` runs on `merge_group`, and has a `ci` job
    /// that needs every other job and runs `if: always()`: with
    /// `standard / pr-checks`, it is one of the repository's two required
    /// checks.
    CiShape = "ci-shape" => ci_shape,
    /// Every `uses:` of a workflow or composite action naming another
    /// repository's action or workflow pins it to a 40-character commit
    /// SHA, followed by a `# v…` comment giving its version.
    ActionsPinned = "actions-pinned" => actions_pinned,
    /// Every workflow's top-level `permissions` is exactly
    /// `contents: read`, or `{}`; a job that needs more sets its own.
    WorkflowPermissions = "workflow-permissions" => workflow_permissions,
    /// Every job of a workflow sets `timeout-minutes`, except one calling a
    /// reusable workflow.
    JobTimeouts = "job-timeouts" => job_timeouts,
    /// Every `Swatinem/rust-cache` step's `save-if` is `false` or
    /// `${{ github.ref == 'refs/heads/main' }}`, and no composite action, nor workflow run by
    /// `pull_request`, `pull_request_target` or `merge_group`, uses
    /// `actions/cache`, which saves on a miss: they use
    /// `actions/cache/restore`, saving with `actions/cache/save` from `main`.
    CacheSaves = "cache-saves" => cache_saves,
    /// A workflow runs `just audit` (or `cargo deny`) on a `schedule` at
    /// least weekly: a cron whose day of the month and month are `*`.
    ScheduledAudit = "scheduled-audit" => scheduled_audit,
    /// `.github/pull_request_template.md` has the headings `## What and
    /// why` and `## Testing`.
    PrTemplate = "pr-template" => pr_template,
    /// With a `book.toml`, every Markdown page under its `src` is linked
    /// from its `SUMMARY.md`, or included in a page with `{{#include …}}`.
    DocsSummary = "docs-summary" => docs_summary,
    /// Every image in a Markdown file, outside code and the skills synced
    /// from this crate, has alt text: `![what it shows](…)`, or an `<img>`
    /// with a non-empty `alt`.
    DocsImages = "docs-images" => docs_images,
    /// No Markdown file but a synced skill's holds, as a word of its prose
    /// (not code, a link's destination, an HTML tag or a URL, nor part of a
    /// name such as `my_lib`, `lib.rs` or `lib@example.com`), a spelling
    /// `standard.toml`'s `[wording]` rejects, such as
    /// `"Source Origin" = "SourceOrigin"`.
    Wording = "wording" => wording,
    /// A module is `foo/mod.rs` only when its directory holds another Rust
    /// file, parsed or not; otherwise it is `foo.rs`. A `mod.rs` beside an
    /// integration test's, benchmark's or example's root, such as
    /// `tests/common/mod.rs`, passes, and so does one under `src/bin`, where
    /// binaries share modules.
    ChildlessModRs = "childless-mod-rs" => childless_mod_rs,
    /// An inline `#[cfg(test)] mod … { }` (or one whose `cfg` requires a
    /// feature named for tests) at the top level of a file a target reaches
    /// outside test code spans fewer than 50 lines or at most 30% of the
    /// file's; a larger one is a file of its own, `mod tests;`.
    InlineTests = "inline-tests" => inline_tests,
    /// `Duration::from_*` with a number, or arithmetic on numbers, sits only
    /// in a `const` or `static` item, a `const fn` or test code: a limit,
    /// timeout or size is a named constant. Test code is what a test
    /// attribute such as `#[tokio::test]`, or a `cfg` requiring `test` or a
    /// feature named for tests (`test-util`), marks, the files such a
    /// module declares, and integration tests and benchmarks.
    InlineLimits = "inline-limits" => inline_limits,
    /// Code outside tests, in the crates `standard.toml`'s `no-json-macro`
    /// lists, invokes no `json!`: it serializes structs. Each name there is
    /// a workspace crate.
    NoJsonMacro = "no-json-macro" => no_json_macro,
    /// When `standard.toml`'s `route-modules` lists modules, such as
    /// `my_app::routes`, code outside them, tests and examples passes no
    /// string literal starting with `/` to `Router::route`, `route_service`,
    /// `nest` or `nest_service`, or to a `Redirect` constructor: it uses the
    /// paths those modules name. Each listed module is a file of a library
    /// or binary.
    RouteLiterals = "route-literals" => route_literals,
    /// No function body outside test code with at least 20 tokens repeats
    /// another, token for token once its local bindings (parameters and
    /// the names its patterns bind) are renamed in the order they appear;
    /// paths, fields, methods and types stay as written.
    DuplicateItems = "duplicate-items" => duplicate_items,
    /// Outside tests, no `tracing` event or span macro (`info!`,
    /// `info_span!`, `event!`, …) records a field whose name, or a run of its
    /// dotted parts, `standard.toml`'s `denied-fields` lists: `query` denies
    /// `db.query.text`, `url.query` only `url.query` and `http.url.query`.
    /// Nor does an `#[instrument]` function, in its `fields(…)` or as an
    /// argument it doesn't `skip`.
    TracingFields = "tracing-fields" => tracing_fields,
    /// Each skill in `.agents/skills` that comes from sourceorigin-standard
    /// (`standard`, `cleanup` and `docs`, and any other whose `SKILL.md`
    /// says it is synced) matches the crate's copy, file for file.
    /// `cargo xtask standard skills` writes them.
    SkillsSynced = "skills-synced" => skills_synced,
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
    /// Two fixes would change one file in ways that conflict, such as two
    /// edits of the same lines.
    #[error("`--fix` would change {} twice: {first}, and {second}", path.display())]
    Conflict {
        /// The file, from the repository's root.
        path: PathBuf,
        /// What the first fix changes.
        first: String,
        /// What the second changes.
        second: String,
    },
    /// A fix couldn't write or delete a file or directory.
    #[error("write {}", path.display())]
    Write {
        /// The file.
        path: PathBuf,
        /// Why it couldn't be written.
        #[source]
        source: std::io::Error,
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
    /// The manifests of the workspace's crates.
    manifests: Vec<TomlFile<Manifest>>,
    /// The lints `[workspace.lints]` sets.
    lint_levels: Vec<LintLevel>,
    /// Every Rust file git tracks or would track that parses, by path.
    sources: Vec<Source>,
    /// Every one that doesn't.
    unparsed: Vec<Unparsed>,
    /// Every file git tracks or would track, from the root.
    files: BTreeSet<PathBuf>,
    /// The configuration files that don't parse.
    unreadable: Unreadable,
    /// Each Rust file's module paths.
    modules: Modules,
    /// Each crate's budget of exceptions.
    budget: Budget,
    /// The other workspaces the repository holds, whose crates have
    /// budgets too.
    other_workspaces: Vec<OtherWorkspace>,
    /// Each manifest's lints set to `allow`, by its path.
    manifest_allows: BTreeMap<String, usize>,
    /// The justfiles, whose recipes the rules read.
    justfiles: Vec<TextFile>,
    /// The workflows and composite actions.
    workflows: Vec<Workflow>,
    /// `t3.json`.
    t3: Option<T3>,
    /// The manifests, justfiles and workflows, with their comments.
    commented: Vec<Commented>,
    /// The root manifest.
    root_manifest: Option<TomlFile<RootManifest>>,
    /// `.cargo/config.toml`.
    cargo_config: Option<TomlFile<CargoConfig>>,
    /// `AGENTS.md`, any `CLAUDE.md`, and the skills.
    agent_files: AgentFiles,
    /// `rust-toolchain.toml`.
    toolchain: Option<TomlFile<ToolchainFile>>,
    /// `rustfmt.toml`, empty when missing, and `None` when it doesn't
    /// parse.
    rustfmt: Option<TomlFile<Settings>>,
    /// `clippy.toml`, empty when missing, and `None` when it doesn't
    /// parse.
    clippy: Option<TomlFile<Settings>>,
    /// `.config/nextest.toml`, empty when missing, and `None` when it
    /// doesn't parse.
    nextest: Option<TomlFile<NextestConfig>>,
    /// `deny.toml`.
    deny: Option<TomlFile<DenyConfig>>,
    /// The justfile, as `just` describes it.
    justfile: Option<Justfile>,
    /// The pull request template.
    pr_template: Option<TextFile>,
    /// mdBook's `book.toml`.
    book: Option<TomlFile<Book>>,
    /// Every Markdown file.
    markdown: Vec<TextFile>,
    /// The skills it holds from the crate.
    synced: Synced,
}

/// What the rules read of a repository: everything, or the file that keeps
/// `cargo metadata` from describing its workspace, such as a manifest that
/// doesn't parse, which is then the one violation.
enum Loaded {
    Repository(Box<Repository>),
    Unreadable(Unparsed),
}

impl Repository {
    /// Loads everything the rules read, so the rules themselves can't
    /// fail.
    fn load(root: &Path) -> Result<Loaded, Error> {
        let metadata = match Metadata::load(Workspace::new(root)) {
            Ok(metadata) => metadata,
            Err(MetadataError::Command(CommandError::Failed { stderr, .. })) => {
                return Ok(Loaded::Unreadable(Unparsed::cargo(root, &stderr)?));
            }
            Err(error) => return Err(error.into()),
        };
        let (sources, unparsed) = Source::load_all(root)?;
        let mut unreadable = Unreadable::default();
        let root_manifest = TomlFile::<RootManifest>::load_root(root, &mut unreadable)?;
        let manifests = TomlFile::load_all(&metadata, &mut unreadable)?;
        let budget = Budget::load(root, &mut unreadable)?;
        let other_workspaces =
            OtherWorkspace::load_all(root, &metadata, &sources, &mut unreadable)?;
        let manifest_allows =
            budget::manifest_allows(root, &metadata, &other_workspaces, &mut unreadable)?;
        unreadable.check_others(root)?;
        Ok(Loaded::Repository(Box::new(Self {
            config: Config::load(root)?,
            manifests,
            lint_levels: root_manifest.as_ref().map_or_else(Vec::new, LintLevel::all),
            modules: Modules::of(&metadata, &sources),
            budget,
            other_workspaces,
            manifest_allows,
            root_manifest,
            cargo_config: TomlFile::load(root, layout::CARGO_CONFIG, &mut unreadable)?,
            workflows: Workflow::load_all(root, &mut unreadable)?,
            t3: T3::load(root, &mut unreadable)?,
            agent_files: AgentFiles::load(root, &mut unreadable)?,
            toolchain: TomlFile::load(root, toolchain::PATH, &mut unreadable)?,
            rustfmt: TomlFile::load_or_empty(root, rustfmt_config::PATH, &mut unreadable)?,
            clippy: TomlFile::load_or_empty(root, clippy_config::PATH, &mut unreadable)?,
            nextest: TomlFile::load_or_empty(root, nextest_config::PATH, &mut unreadable)?,
            deny: TomlFile::load(root, deny_config::PATH, &mut unreadable)?,
            justfile: Justfile::load(root),
            pr_template: TextFile::load_all(root, &[pr_template::PATH], &mut unreadable)?.pop(),
            book: TomlFile::load(root, docs_summary::BOOK, &mut unreadable)?,
            markdown: TextFile::load_all(root, &["*.md"], &mut unreadable)?,
            commented: Commented::load_all(root)?,
            justfiles: TextFile::load_all(root, &cargo_locked::JUSTFILES, &mut unreadable)?,
            synced: Synced::load(root)?,
            metadata,
            files: Workspace::new(root)
                .files(&[])?
                .into_iter()
                .map(PathBuf::from)
                .collect(),
            sources,
            unparsed,
            unreadable,
        })))
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

    /// The fixes of every rule, except those resolving a file or crate its
    /// deviation excuses, and the removal of each deviation, or entry of
    /// one, that excuses nothing.
    fn fixes(&self) -> Vec<Fix> {
        let mut fixes = Vec::new();
        for &rule in Rule::ALL {
            let checked = rule.check(self);
            let Some(deviation) = self.config.deviation(rule) else {
                fixes.extend(checked.fixes);
                continue;
            };
            fixes.extend(
                checked
                    .fixes
                    .into_iter()
                    .filter(|fix| !deviation.excuses(&fix.resolves, self.crate_of(&fix.resolves))),
            );
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
            .partition(|finding| deviation.excuses(&finding.path, self.crate_of_finding(finding)));
        let excused: Vec<(&Path, Option<&str>)> = excused
            .iter()
            .map(|finding| (finding.path.as_path(), self.crate_of_finding(finding)))
            .collect();
        (kept, deviation.stale(&excused))
    }

    /// The crate `finding` is about: the one owning its file, or for one
    /// in `budget.toml`, the crate its line budgets.
    fn crate_of_finding(&self, finding: &Finding) -> Option<&str> {
        self.crate_of(&finding.path).or_else(|| {
            (finding.path == Path::new(budget::PATH))
                .then(|| self.budget.key_at(finding.line))
                .flatten()
        })
    }

    /// Every Rust file, from the repository's root, whether it parses or
    /// not.
    fn rust_files(&self) -> impl Iterator<Item = &Path> {
        self.sources
            .iter()
            .map(|source| source.path.as_path())
            .chain(self.unparsed.iter().map(|unparsed| unparsed.path.as_path()))
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
    Ok(match Repository::load(root)? {
        Loaded::Repository(repository) => repository.violations(),
        Loaded::Unreadable(unparsed) => vec![unparsed.finding().of(Rule::ConfigSyntax)],
    })
}

/// Makes every mechanical fix the rules know in the repository at `root`,
/// such as adding `[lints] workspace = true` or lowering a budget of
/// `budget.toml` to what the crate holds, and returns them. Each is an edit
/// to a TOML document, which keeps the file's layout and comments, or to a
/// line of another file, such as a justfile. What needs judgement stays a
/// violation for [`check`] to report.
///
/// # Errors
///
/// When a file a rule reads can't be loaded, or a fixed file written or
/// deleted.
pub fn fix(root: &Path) -> Result<Vec<Fix>, Error> {
    let Loaded::Repository(repository) = Repository::load(root)? else {
        return Ok(Vec::new());
    };
    let mut fixes = repository.fixes();
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
    Ok(match Repository::load(root)? {
        Loaded::Repository(repository) => repository.report(),
        Loaded::Unreadable(unparsed) => {
            Report::unreadable(unparsed.finding().of(Rule::ConfigSyntax))
        }
    })
}

/// Writes the crate's skills (`standard`, `cleanup` and `docs`) into
/// `.agents/skills` of the repository at `root`, removes the files of
/// synced skills the crate no longer ships, and returns what it changed.
/// `cargo xtask standard skills` calls it; [`fix`] does it too.
///
/// # Errors
///
/// When `.agents/skills` can't be read, or a file written or removed.
pub fn sync_skills(root: &Path) -> Result<Vec<Fix>, Error> {
    let mut fixes = Synced::load(root)?.checked().fixes;
    Fix::apply_all(root, &mut fixes)?;
    Ok(fixes)
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
