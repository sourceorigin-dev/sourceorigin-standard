//! A fixture workspace the rules check, shared by the tests of each rule.

mod actions_pinned;
mod agent_files;
mod agents_md;
mod anyhow_edges;
mod budget;
mod cargo_locked;
mod ci_shape;
mod clippy_config;
mod comments;
mod config_comments;
mod config_syntax;
mod deny_config;
mod fix;
mod framework;
mod generated_lints;
mod layout;
mod nextest_config;
mod recipes;
mod recipes_only;
mod report;
mod rustfmt_config;
mod t3_setup;
mod toolchain;
mod unsafe_modules;
mod workspace_dependencies;
mod workspace_lints;
mod workspace_manifest;

use super::Rule;
use crate::testing::Repository;

/// A Cargo workspace whose members are `crates/*`, in a scratch git
/// repository.
struct Fixture(Repository);

impl Fixture {
    fn new() -> Self {
        let repository = Repository::new();
        repository.write(
            "Cargo.toml",
            &format!("[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\n\n{LINTS}"),
        );
        for (path, text) in CONFIG {
            repository.write(path, text);
        }
        Self(repository)
    }

    /// Adds the library crate `name`, its manifest ending with `manifest`.
    fn add_crate(&self, name: &str, manifest: &str) -> &Self {
        self.write(
            &format!("crates/{name}/Cargo.toml"),
            &format!("[package]\nname = \"{name}\"\nedition = \"2024\"\n{manifest}"),
        );
        self.write(&format!("crates/{name}/src/lib.rs"), "");
        self
    }

    fn write(&self, path: &str, text: &str) -> &Self {
        self.0.write(path, text);
        self
    }

    /// Removes the file at `path`, such as one of [`CONFIG`].
    fn remove(&self, path: &str) -> &Self {
        std::fs::remove_file(self.0.root().join(path)).expect("the file is removed");
        self
    }

    /// Makes the fixes of `rule` alone.
    fn fix_rule(&self, rule: Rule) -> &Self {
        let Ok(super::Loaded::Repository(repository)) = super::Repository::load(self.0.root())
        else {
            panic!("the repository loads");
        };
        super::Fix::apply_all(self.0.root(), &mut rule.check(&repository).fixes)
            .expect("the fixes are made");
        self
    }

    /// The violations of `rule`, each as `path:line: message (rule)`.
    fn violations(&self, rule: Rule) -> Vec<String> {
        super::check(self.0.root())
            .expect("the rules run")
            .iter()
            .filter(|violation| violation.rule == rule)
            .map(|violation| {
                format!(
                    "{}:{}: {} ({})",
                    violation.path.display(),
                    violation.line,
                    violation.message,
                    rule.name()
                )
            })
            .collect()
    }
}

/// The standard's lint block, which every fixture workspace sets.
const LINTS: &str = "[workspace.lints.rust]
unsafe_code = \"warn\"
missing_docs = \"warn\"

[workspace.lints.clippy]
all = { level = \"warn\", priority = -1 }
pedantic = { level = \"warn\", priority = -1 }
unwrap_used = \"warn\"
expect_used = \"warn\"
panic = \"warn\"
todo = \"warn\"
unimplemented = \"warn\"
dbg_macro = \"warn\"
allow_attributes = \"warn\"
allow_attributes_without_reason = \"warn\"
undocumented_unsafe_blocks = \"warn\"
self_named_module_files = \"warn\"
";

/// The configuration files every fixture holds, as the standard has them,
/// so a rule's fixes touch only what a test writes.
const CONFIG: [(&str, &str); 4] = [
    (
        ".cargo/config.toml",
        "[alias]\nxtask = \"run --locked --package xtask --\"\n",
    ),
    (
        "rust-toolchain.toml",
        "[toolchain]\nchannel = \"stable\"\ncomponents = [\"rustfmt\", \"clippy\"]\n",
    ),
    (
        "rustfmt.toml",
        "edition = \"2024\"\nnewline_style = \"Unix\"\nuse_field_init_shorthand = true\n",
    ),
    (
        "clippy.toml",
        "allow-expect-in-tests = true\nallow-panic-in-tests = true\nallow-dbg-in-tests = true\n",
    ),
];

/// A manifest tail inheriting the workspace's lints.
const INHERITS: &str = "[lints]\nworkspace = true\n";
