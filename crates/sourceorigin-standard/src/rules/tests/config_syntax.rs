use super::Fixture;
use crate::rules::skills::MARKER;
use crate::rules::{self, Rule};

#[test]
fn a_config_file_that_doesnt_parse_is_a_violation_and_the_rules_go_on() {
    let fixture = Fixture::new();
    fixture
        .add_crate("bare", "")
        .write("deny.toml", "[bans]\nwildcards = \"deny\"\n[licenses\n")
        .write("rustfmt.toml", "edition = \"2024\"\n")
        .write("budget.toml", "[expect]\nbare = \n")
        .write(".github/workflows/ci.yml", "on: push\njobs: [\n")
        .write("t3.json", "{\n  \"scripts\": [\n}\n")
        .write("justfile", "ok:\n    echo\n\nbad recipe(:\n");
    let violations: Vec<String> = fixture
        .violations(Rule::ConfigSyntax)
        .iter()
        .map(|violation| violation.split(": ").next().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(
        violations,
        [
            ".github/workflows/ci.yml:3",
            "budget.toml:2",
            "deny.toml:3",
            "justfile:4",
            "t3.json:3",
        ]
    );
    assert_eq!(fixture.violations(Rule::WorkspaceLints).len(), 1);
}

#[test]
fn a_manifest_cargo_cant_read_is_the_one_violation() {
    let fixture = Fixture::new();
    fixture
        .add_crate("bare", "")
        .write("crates/bare/Cargo.toml", "[package\nname = \"bare\"\n");
    let violations = rules::check(fixture.0.root()).expect("the rules run");
    assert_eq!(
        violations
            .iter()
            .map(|violation| (
                violation.rule,
                violation.path.to_string_lossy(),
                violation.line
            ))
            .collect::<Vec<_>>(),
        [(Rule::ConfigSyntax, "crates/bare/Cargo.toml".into(), 1)]
    );
    assert!(
        rules::fix(fixture.0.root())
            .expect("nothing to fix")
            .is_empty()
    );
}

#[test]
fn a_cargo_configuration_cargo_cant_read_is_the_one_violation() {
    let fixture = Fixture::new();
    fixture
        .add_crate("bare", "")
        .write(".cargo/config.toml", "[alias]\nx = \"y\"\n[build\n");
    let violations = rules::check(fixture.0.root()).expect("the rules run");
    assert_eq!(
        violations
            .iter()
            .map(|violation| (
                violation.path.to_string_lossy().into_owned(),
                violation.line
            ))
            .collect::<Vec<_>>(),
        [(".cargo/config.toml".to_owned(), 3)]
    );
}

#[test]
fn a_message_naming_no_file_lands_on_the_root_manifest_and_one_naming_a_member_on_its_manifest() {
    let fixture = Fixture::new();
    fixture
        .add_crate("bare", "")
        .write(
            "crates/bare/Cargo.toml",
            "[package]\nname = \"bare\"\nedition = [\n",
        )
        .write("fixtures/broken/Cargo.toml", "[package\n");
    let place = |stderr: &str| {
        let unparsed = super::super::source::Unparsed::cargo(fixture.0.root(), stderr)
            .expect("the place is found");
        (unparsed.path.to_string_lossy().into_owned(), unparsed.line)
    };
    for stderr in ["error: failed to load manifest for workspace member", ""] {
        assert_eq!(place(stderr), ("Cargo.toml".to_owned(), 1), "{stderr:?}");
    }
    assert_eq!(
        place("error: failed to load manifest for workspace member `crates/bare`"),
        ("crates/bare/Cargo.toml".to_owned(), 3)
    );
}

#[test]
fn a_manifest_cargo_refuses_though_it_parses_is_the_one_cargo_names_with_its_whole_message() {
    let fixture = Fixture::new();
    fixture
        .add_crate("bare", "")
        .write(
            "crates/bare/Cargo.toml",
            "[package]\nname = \"bare\"\nedition = \"2077\"\n",
        )
        .write("fixtures/broken/Cargo.toml", "[package\n");
    let violations = rules::check(fixture.0.root()).expect("the rules run");
    let [violation] = violations.as_slice() else {
        panic!("one violation: {violations:?}");
    };
    assert_eq!(
        (
            violation.rule,
            violation.path.to_string_lossy(),
            violation.line
        ),
        (Rule::ConfigSyntax, "crates/bare/Cargo.toml".into(), 1)
    );
    assert!(
        violation.message.contains("Caused by:") && violation.message.contains("2077"),
        "{}",
        violation.message
    );
}

#[test]
fn the_innermost_file_cargo_names_is_the_one_and_a_warning_names_none() {
    let fixture = Fixture::new();
    fixture
        .add_crate("b", "\n[dependencies]\nz = { path = \"../z\" }\n")
        .write(
            "crates/z/Cargo.toml",
            "[package]\nname = \"z\"\nedition = \"2077\"\n",
        )
        .write("crates/z/src/lib.rs", "")
        .write(".cargo/config", "[build]\n");
    let violations = rules::check(fixture.0.root()).expect("the rules run");
    assert_eq!(
        violations
            .iter()
            .map(|violation| violation.path.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        ["crates/z/Cargo.toml"]
    );
    let unparsed = super::super::source::Unparsed::cargo(
        fixture.0.root(),
        "warning: `.cargo/config` is deprecated in favor of `config.toml`\n\
         error: something cargo couldn't read",
    )
    .expect("the place is found");
    assert_eq!(unparsed.path.to_string_lossy(), "Cargo.toml");
}

/// The violations a fixture holds when `path`, `prefix` then `line` with
/// its `cafe` not UTF-8, beside `others`, that it doesn't when `line` is as
/// written, valid for the file's format, as `path:line [rule] message`. Its
/// other rules run, and `--fix` runs.
fn not_utf8_after(path: &str, prefix: &str, line: &str, others: &[(&str, &str)]) -> Vec<String> {
    let violations = |line: &[u8]| {
        let fixture = Fixture::new();
        fixture.add_crate("bare", "");
        for (other, text) in others {
            fixture.write(other, text);
        }
        let file = fixture.0.root().join(path);
        std::fs::create_dir_all(file.parent().expect("the file has a directory"))
            .expect("the directory is made");
        std::fs::write(&file, [prefix.as_bytes(), line].concat()).expect("the file is written");
        let violations: Vec<String> = rules::check(fixture.0.root())
            .expect("the rules run")
            .iter()
            .map(|violation| {
                format!(
                    "{}:{} [{}] {}",
                    violation.path.display(),
                    violation.line,
                    violation.rule.name(),
                    violation.message
                )
            })
            .collect();
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("[workspace-lints]")),
            "the other rules went on: {violations:?}"
        );
        assert!(rules::fix(fixture.0.root()).is_ok(), "--fix runs");
        violations
    };
    let clean = violations(line.as_bytes());
    let (before, after) = line.split_once("cafe").expect("the line holds `cafe`");
    violations(&[before.as_bytes(), b"caf\xe9", after.as_bytes()].concat())
        .into_iter()
        .filter(|violation| !clean.contains(violation))
        .collect()
}

/// The violations a fixture holds when `path`'s second line, a `#`
/// comment, isn't UTF-8, beside `others`, that it doesn't when it is.
fn not_utf8_with(path: &str, others: &[(&str, &str)]) -> Vec<String> {
    not_utf8_after(path, "# a file\n", "# cafe\n", others)
}

/// The violations a fixture holds when `path`'s second line isn't UTF-8.
fn not_utf8(path: &str) -> Vec<String> {
    not_utf8_with(path, &[])
}

/// The one `config-syntax` violation a `path` that isn't UTF-8 from its
/// `line` is.
fn not_utf8_at(path: &str, line: usize) -> Vec<String> {
    vec![format!(
        "{path}:{line} [config-syntax] it isn't UTF-8 text: fix it so the rules can read it"
    )]
}

/// The one `config-syntax` violation a `path` that isn't UTF-8 from its
/// second line is.
fn not_utf8_violation(path: &str) -> Vec<String> {
    not_utf8_at(path, 2)
}

#[test]
fn a_workflow_that_isnt_utf8_is_one_violation() {
    let path = ".github/workflows/ci.yml";
    assert_eq!(not_utf8(path), not_utf8_violation(path));
}

#[test]
fn a_just_module_that_isnt_utf8_is_one_violation() {
    let path = "tasks.just";
    assert_eq!(not_utf8(path), not_utf8_violation(path));
}

#[test]
fn a_toml_file_that_isnt_utf8_is_one_violation() {
    let path = "deny.toml";
    assert_eq!(not_utf8(path), not_utf8_violation(path));
}

#[test]
fn t3_json_that_isnt_utf8_is_one_violation() {
    let path = "t3.json";
    assert_eq!(
        not_utf8_after(
            path,
            "{\n",
            "  \"scripts\": [], \"name\": \"cafe\"\n}\n",
            &[]
        ),
        not_utf8_violation(path)
    );
}

#[test]
fn a_rust_file_that_isnt_utf8_is_one_violation() {
    assert_eq!(
        not_utf8_after(
            "crates/bare/src/lib.rs",
            "//! A crate.\n",
            "//! cafe\n",
            &[]
        ),
        [
            "crates/bare/src/lib.rs:2 [rust-syntax] it isn't UTF-8 text: fix it, or deviate by path \
          for a fixture that mustn't parse"
        ]
    );
}

#[test]
fn a_justfile_that_isnt_utf8_is_one_violation() {
    let path = "justfile";
    assert_eq!(not_utf8(path), not_utf8_violation(path));
}

#[test]
fn a_module_the_justfile_imports_that_isnt_utf8_is_one_violation() {
    let path = "tasks.just";
    assert_eq!(
        not_utf8_with(
            path,
            &[("justfile", "import 'tasks.just'\n\nok:\n    echo ok\n")]
        ),
        not_utf8_violation(path)
    );
}

#[test]
fn agents_md_that_isnt_utf8_is_one_violation() {
    let path = "AGENTS.md";
    assert_eq!(not_utf8(path), not_utf8_violation(path));
}

#[test]
fn a_pull_request_template_that_isnt_utf8_is_one_violation_and_fix_runs() {
    let path = ".github/pull_request_template.md";
    assert_eq!(not_utf8(path), not_utf8_violation(path));
}

#[test]
fn a_synced_skill_file_that_isnt_utf8_is_one_violation_which_fix_rewrites() {
    let path = ".agents/skills/standard/SKILL.md";
    let prefix = format!("---\nname: standard\n---\n\n{MARKER}\n");
    assert_eq!(
        not_utf8_after(path, &prefix, "# cafe\n", &[]),
        not_utf8_at(path, 6)
    );
    let fixture = Fixture::new();
    fixture.add_crate("bare", "");
    std::fs::write(
        fixture.0.root().join(path),
        [prefix.as_bytes(), b"caf\xe9\n"].concat(),
    )
    .expect("the file is written");
    rules::fix(fixture.0.root()).expect("the fixes are made");
    let rewritten = std::fs::read_to_string(fixture.0.root().join(path))
        .expect("the skill file is UTF-8 text again");
    assert!(rewritten.contains(MARKER), "{rewritten}");
}

#[test]
fn a_book_summary_that_isnt_utf8_is_one_violation() {
    let path = "docs/SUMMARY.md";
    let others = [
        ("book.toml", "[book]\nsrc = \"docs\"\n"),
        ("docs/a.md", "# A\n"),
    ];
    assert_eq!(
        not_utf8_after(path, "# Summary\n- [A](a.md)\n", "# cafe\n", &others),
        not_utf8_at(path, 3)
    );
}

#[test]
fn a_justfile_a_module_loads_that_isnt_utf8_is_one_violation() {
    let path = "sub/justfile";
    assert_eq!(
        not_utf8_with(path, &[("justfile", "mod sub\n\nok:\n    echo ok\n")]),
        not_utf8_violation(path)
    );
}

#[test]
fn a_book_page_that_includes_another_and_isnt_utf8_is_one_violation() {
    let path = "docs/a.md";
    let others = [
        ("book.toml", "[book]\nsrc = \"docs\"\n"),
        ("docs/SUMMARY.md", "# Summary\n- [A](a.md)\n"),
        ("docs/part.md", "Part.\n"),
    ];
    assert_eq!(
        not_utf8_after(path, "# A\n{{#include part.md}}\n", "cafe\n", &others),
        not_utf8_at(path, 3)
    );
}

#[test]
fn a_deviation_excusing_a_file_that_isnt_utf8_stays() {
    let path = ".github/pull_request_template.md";
    let others = [(
        "standard.toml",
        "[deviations]\npr-template = { reason = \"r\", paths = [\".github/pull_request_template.md\"] }\n",
    )];
    assert_eq!(
        not_utf8_after(path, "# a file\n", "cafe\n", &others),
        not_utf8_violation(path)
    );
}

#[test]
fn a_budget_that_isnt_utf8_is_one_violation() {
    let path = "budget.toml";
    let others = [(
        "crates/bare/src/lib.rs",
        "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n",
    )];
    assert_eq!(
        not_utf8_after(path, "[expect]\nbare = 1\n", "# cafe\n", &others),
        not_utf8_at(path, 3)
    );
}

/// The violations, as `path:line [rule]`, of a fixture holding `excused`, a
/// module that isn't UTF-8 text, with `others` beside it.
fn beside_an_excused_fixture(excused: &str, others: &[(&str, &str)]) -> Vec<String> {
    let fixture = Fixture::new();
    fixture.add_crate("bare", "");
    for (path, text) in others {
        fixture.write(path, text);
    }
    let file = fixture.0.root().join(excused);
    std::fs::create_dir_all(file.parent().expect("the fixture has a directory"))
        .expect("the directory is made");
    std::fs::write(file, b"# caf\xe9\n").expect("the fixture is written");
    rules::check(fixture.0.root())
        .expect("the rules run")
        .iter()
        .map(|violation| {
            format!(
                "{}:{} [{}]",
                violation.path.display(),
                violation.line,
                violation.rule.name()
            )
        })
        .collect()
}

#[test]
fn an_excused_file_that_isnt_utf8_hides_no_other_justfile_error() {
    let violations = beside_an_excused_fixture(
        "fixtures/latin1.just",
        &[
            (
                "standard.toml",
                "[deviations]\nconfig-syntax = { reason = \"r\", paths = [\"fixtures/**\"] }\n",
            ),
            ("justfile", "ok:\n    echo\n\nbad recipe(:\n"),
        ],
    );
    assert!(
        violations.contains(&"justfile:4 [config-syntax]".to_owned()),
        "{violations:?}"
    );
}

#[test]
fn an_excused_file_that_isnt_utf8_hides_no_other_stale_deviation() {
    for (excused, deviation) in [
        (
            "fixtures/latin1.just",
            "wording = { reason = \"r\", paths = [\"nowhere.md\"] }",
        ),
        ("fixtures/latin1.just", "wording = \"r\""),
        (
            "crates/bare/fixtures/latin1.just",
            "wording = { reason = \"r\", crates = [\"bare\"] }",
        ),
    ] {
        let standard = format!(
            "[deviations]\nconfig-syntax = {{ reason = \"r\", paths = [\"{excused}\"] }}\n{deviation}\n"
        );
        let violations = beside_an_excused_fixture(excused, &[("standard.toml", &standard)]);
        assert!(
            violations.contains(&"standard.toml:3 [wording]".to_owned()),
            "{deviation}: {violations:?}"
        );
    }
}

#[test]
fn an_unparsed_budget_keeps_the_deviation_excusing_its_crate() {
    let path = "budget.toml";
    let others = [
        (
            "crates/bare/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n",
        ),
        (
            "standard.toml",
            "[deviations]\nbudget = { reason = \"r\", crates = [\"bare\"] }\n",
        ),
    ];
    assert_eq!(
        not_utf8_after(path, "[expect]\n", "# cafe\n", &others),
        not_utf8_at(path, 2)
    );
}
