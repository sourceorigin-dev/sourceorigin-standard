use std::path::{Path, PathBuf};

use super::{Fixture, INHERITS, LINTS};
use crate::rules::fix::Fix;
use crate::rules::{Error, Rule, fix};
use crate::testing::Repository;

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

/// Makes `fixes` in a scratch repository holding `existing`, and returns
/// its files then, or the error.
fn apply(existing: &[(&str, &str)], mut fixes: Vec<Fix>) -> Result<Vec<(String, String)>, Error> {
    let repository = Repository::new();
    for (path, text) in existing {
        repository.write(path, text);
    }
    Fix::apply_all(repository.root(), &mut fixes)?;
    let mut found = Vec::new();
    for entry in walk(repository.root()) {
        let path = entry
            .strip_prefix(repository.root())
            .expect("under the root")
            .to_string_lossy()
            .into_owned();
        if !path.starts_with(".git") {
            found.push((
                path,
                std::fs::read_to_string(&entry).expect("the file is read"),
            ));
        }
    }
    found.sort();
    Ok(found)
}

/// Every file under `dir`.
fn walk(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .expect("the directory is read")
        .flat_map(|entry| {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}

fn owned(files: &[(&str, &str)]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect()
}

#[test]
fn line_edits_apply_against_the_original_lines_and_keep_crlf() {
    let files = apply(
        &[("justfile", "one\r\ntwo\r\nthree\r\nfour\r\n")],
        vec![
            Fix::lines("justfile", "replace".to_owned(), 2..4, "2\n3".to_owned()),
            Fix::lines("justfile", "insert".to_owned(), 1..1, "zero".to_owned()),
            Fix::lines("justfile", "delete".to_owned(), 4..5, String::new()),
            Fix::append("justfile", "append".to_owned(), "five".to_owned()),
            Fix::append("new/notes.txt", "create".to_owned(), "first".to_owned()),
        ],
    )
    .expect("the fixes are made");
    assert_eq!(
        files,
        owned(&[
            ("justfile", "zero\r\none\r\n2\r\n3\r\nfive\r\n"),
            ("new/notes.txt", "first\n"),
        ])
    );
}

#[test]
fn files_are_written_deleted_and_moved_and_emptied_directories_go() {
    let files = apply(
        &[
            ("old/deep/gone.rs", "gone"),
            ("src/foo/mod.rs", "foo"),
            ("src/lib.rs", "lib"),
        ],
        vec![
            Fix::write("docs/new.md", "write".to_owned(), "new\n".to_owned()),
            Fix::delete("old/deep/gone.rs", "delete".to_owned()),
            Fix::move_to(
                "src/foo/mod.rs",
                "move".to_owned(),
                PathBuf::from("src/foo.rs"),
            ),
            Fix::lines(
                "src/foo/mod.rs",
                "edit".to_owned(),
                1..2,
                "moved".to_owned(),
            ),
        ],
    )
    .expect("the fixes are made");
    assert_eq!(
        files,
        owned(&[
            ("docs/new.md", "new\n"),
            ("src/foo.rs", "moved\n"),
            ("src/lib.rs", "lib"),
        ])
    );
}

#[test]
fn conflicting_fixes_are_an_error_and_change_nothing() {
    let cases: Vec<Vec<Fix>> = vec![
        vec![
            Fix::lines("justfile", "first".to_owned(), 1..3, String::new()),
            Fix::lines("justfile", "second".to_owned(), 2..4, String::new()),
        ],
        vec![
            Fix::toml("justfile", "first".to_owned(), |_| {}),
            Fix::lines("justfile", "second".to_owned(), 1..2, String::new()),
        ],
        vec![
            Fix::write("justfile", "first".to_owned(), String::new()),
            Fix::delete("justfile", "second".to_owned()),
        ],
        vec![
            Fix::move_to("justfile", "first".to_owned(), PathBuf::from("other")),
            Fix::write("other", "second".to_owned(), String::new()),
        ],
    ];
    for fixes in cases {
        let result = apply(
            &[("justfile", "one\ntwo\nthree\n"), ("other.txt", "")],
            fixes,
        );
        assert!(
            matches!(&result, Err(Error::Conflict { first, second, .. }) if first == "first" && second == "second"),
            "{result:?}"
        );
    }
}

#[test]
fn a_scoped_deviation_drops_only_the_fixes_of_its_scope() {
    let fixture = Fixture::new();
    fixture
        .add_crate("generated", "")
        .add_crate("written", "")
        .write(
            "standard.toml",
            "[deviations.workspace-lints]\nreason = \"generated\"\npaths = [\"crates/generated/**\"]\n",
        );
    assert_eq!(
        fixture.fix(),
        ["crates/written/Cargo.toml: add `[lints] workspace = true`"]
    );
}
