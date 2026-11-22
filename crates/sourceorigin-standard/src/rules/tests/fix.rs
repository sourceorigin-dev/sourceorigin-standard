use super::{Fixture, INHERITS, LINTS};
use crate::rules::{Rule, fix};

impl Fixture {
    /// Makes every fix, and returns what each changed.
    fn fix(&self) -> Vec<String> {
        fix(self.0.root())
            .expect("the fixes are made")
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.0.root().join(path)).expect("the file is read")
    }
}

#[test]
fn fix_makes_every_mechanical_fix_and_says_what_it_changed() {
    let fixture = Fixture::new();
    let lints = LINTS
        .replace("unsafe_code = \"warn\"", "unsafe_code = \"forbid\"")
        .replace("missing_docs = \"warn\"", "missing_docs = \"allow\"")
        .replace(
            "pedantic = { level = \"warn\", priority = -1 }",
            "pedantic = { level = \"deny\", priority = -1 } # until it builds",
        );
    fixture
        .add_crate("core", INHERITS)
        .add_crate("bare", "")
        .write(
            "Cargo.toml",
            &format!(
                "[workspace]\nmembers = [\"crates/*\"]\n\n{lints}module_name_repetitions = \
                 \"allow\"\n"
            ),
        )
        .write(
            "crates/core/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n",
        )
        .write(
            "budget.toml",
            "expect = { core = 3, removed = 2 } # the FFI layer\r\ninvariant.core = 2\r\n",
        )
        .write(
            "standard.toml",
            "deviations = { doc-length = \"long docs\", rust-comments = \"vendored\" }\n",
        )
        .write("crates/core/src/vendored.rs", "// Vendored.\n");
    assert_eq!(
        fixture.fix(),
        [
            "crates/bare/Cargo.toml: add `[lints] workspace = true`",
            "Cargo.toml: make `clippy::pedantic` `warn`",
            "Cargo.toml: make `unsafe_code` `warn`",
            "budget.toml: lower `core` under `[expect]` from 3 to 1",
            "budget.toml: remove `removed`, no workspace crate, from `[expect]`",
            "budget.toml: lower `core` under `[invariant]` from 2 to 0",
            "standard.toml: remove the stale deviation from `doc-length`",
        ]
    );
    assert_eq!(
        fixture.read("budget.toml"),
        "expect = { core = 1 } # the FFI layer\r\ninvariant.core = 0\r\n"
    );
    assert_eq!(
        fixture.read("standard.toml"),
        "deviations = { rust-comments = \"vendored\" }\n"
    );
    let manifest = fixture.read("Cargo.toml");
    assert!(
        manifest.contains("pedantic = { level = \"warn\", priority = -1 } # until it builds")
            && manifest.contains("missing_docs = \"allow\"")
            && manifest.contains("module_name_repetitions = \"allow\""),
        "{manifest}"
    );
    for rule in [Rule::Budget, Rule::DocLength, Rule::RustComments] {
        assert_eq!(fixture.violations(rule), Vec::<String>::new(), "{rule:?}");
    }
    assert_eq!(
        fixture.violations(Rule::WorkspaceLints),
        [
            "Cargo.toml:6: `missing_docs` is `allow`, but the standard requires it: make it `warn` \
          (workspace-lints)"
        ]
    );
}

#[test]
fn fix_removes_only_the_stale_entries_of_a_scoped_deviation() {
    let fixture = Fixture::new();
    fixture
        .add_crate("generated", "")
        .add_crate("clean", INHERITS)
        .write(
            "standard.toml",
            "[deviations.workspace-lints]\nreason = \"generated\"\n\
             paths = [\"crates/generated/**\", \"crates/gone/**\"] # both\n\
             crates = [\"clean\"]\n",
        );
    assert_eq!(
        fixture.fix(),
        [
            "standard.toml: remove `crates/gone/**`, `clean` from the deviation from \
          `workspace-lints`"
        ]
    );
    assert_eq!(
        fixture.read("standard.toml"),
        "[deviations.workspace-lints]\nreason = \"generated\"\n\
         paths = [\"crates/generated/**\"] # both\ncrates = []\n"
    );
    assert_eq!(
        fixture.violations(Rule::WorkspaceLints),
        Vec::<String>::new()
    );
}
