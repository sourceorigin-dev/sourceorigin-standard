use super::{Fixture, INHERITS, LINTS};
use crate::rules::Rule;

#[test]
fn every_crate_spends_exactly_its_budget() {
    let fixture = Fixture::new();
    fixture
        .add_crate("exact", INHERITS)
        .add_crate("under", INHERITS)
        .add_crate("over", INHERITS)
        .add_crate("unlisted", INHERITS)
        .write(
            "crates/exact/src/lib.rs",
            "#![expect(clippy::pedantic, reason = \"r\")]\n\
             #[cfg_attr(test, expect(clippy::panic, reason = \"r\"))]\n\
             fn f() {\n    #[expect(unused, unused_variables, reason = \"r\")]\n    let x = 0;\n}\n",
        )
        .write(
            "crates/under/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {\n    // INVARIANT: one.\n    \
             let a = 0;\n    // INVARIANT: two,\n    // on two lines.\n    let b = 0;\n    \
             // SAFETY: a marked comment,\n    // INVARIANT: and a third invariant.\n    \
             let c = 0;\n}\n",
        )
        .write(
            "crates/over/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n#[allow(dead_code)]\nfn g() {}\n\
             #[expect(dead_code, reason = \"r\")]\nfn h() {}\n",
        )
        .write(
            "crates/unlisted/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n",
        )
        .write(
            "budget.toml",
            "[expect]\nexact = 4\nunder = 4\nover = 1\nremoved = 2\n\n[invariant]\nunder = 1\n",
        );
    assert_eq!(
        fixture.violations(Rule::Budget),
        [
            "budget.toml:3: `under` has 1 `#[expect]` lints, under its budget of 4: lower \
             `under` under `[expect]` in budget.toml to 1, as `--fix` does (budget)",
            "budget.toml:4: `over` has 2 `#[expect]` lints, over its budget of 1: remove one, \
             or raise `over` under `[expect]` in budget.toml, which the owner reviews (budget)",
            "budget.toml:5: `removed` is no crate of the repository: remove it from `[expect]` \
             (budget)",
            "budget.toml:8: `under` has 3 `// INVARIANT:` lines, over its budget of 1: remove \
             one, or raise `under` under `[invariant]` in budget.toml, which the owner reviews \
             (budget)",
            "crates/unlisted/Cargo.toml:1: `unlisted` has 1 `#[expect]` lints, over its budget \
             of 0: remove one, or raise `unlisted` under `[expect]` in budget.toml, which the \
             owner reviews (budget)",
        ]
    );
}

#[test]
fn a_macro_body_counts_and_a_file_no_target_reaches_doesnt() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "crates/core/src/lib.rs",
            "macro_rules! m {\n    () => {\n        #[expect(dead_code, reason = \"r\")]\n        \
             fn f() {}\n    };\n}\n",
        )
        .write(
            "crates/core/tests/fixtures/case.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n",
        )
        .write("budget.toml", "[expect]\ncore = 1\n");
    assert_eq!(fixture.violations(Rule::Budget), [] as [&str; 0]);
}

#[test]
fn another_workspaces_crates_have_budgets_but_not_their_ui_cases() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "tools/own/Cargo.toml",
            "[package]\nname = \"own\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .write(
            "tools/own/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n",
        )
        .write(
            "tools/lints/Cargo.toml",
            "[package]\nname = \"lints\"\nedition = \"2024\"\n\n[[example]]\nname = \"case\"\n\
             path = \"ui/case.rs\"\n\n[dev-dependencies]\ndylint_testing = { path = \"testing\" }\n\n\
             [workspace]\n",
        )
        .write("tools/lints/src/lib.rs", "")
        .write(
            "tools/lints/ui/case.rs",
            "#[expect(dead_code, reason = \"the case\")]\nfn f() {}\nfn main() {}\n",
        )
        .write(
            "tools/lints/testing/Cargo.toml",
            "[package]\nname = \"dylint_testing\"\nedition = \"2024\"\n",
        )
        .write("tools/lints/testing/src/lib.rs", "");
    for manifest in ["tools/own/Cargo.toml", "tools/lints/Cargo.toml"] {
        std::process::Command::new("cargo")
            .args([
                "generate-lockfile",
                "--offline",
                "--manifest-path",
                manifest,
            ])
            .current_dir(fixture.0.root())
            .status()
            .expect("cargo locks the other workspace");
    }
    assert_eq!(
        fixture.violations(Rule::Budget),
        [
            "tools/own/Cargo.toml:1: `own` has 1 `#[expect]` lints, over its budget of 0: remove \
          one, or raise `own` under `[expect]` in budget.toml, which the owner reviews (budget)"
        ]
    );
}

#[test]
fn a_manifests_allowed_lints_have_a_budget_in_any_form() {
    let fixture = Fixture::new();
    fixture
        .write(
            "Cargo.toml",
            &format!(
                "[workspace]\nresolver = \"3\"\nmembers = [\"crates/*\"]\n\n{LINTS}\
                 too_many_arguments = {{ level = \"\\u0061llow\", priority = 1 }}\n\n\
                 [workspace.lints.rustdoc]\nbroken_intra_doc_links = \"allow\"\n"
            ),
        )
        .add_crate("core", INHERITS)
        .write(
            "budget.toml",
            "[allow]\n\"Cargo.toml\" = 1\n\"gone/Cargo.toml\" = 1\n",
        );
    assert_eq!(
        fixture.violations(Rule::Budget),
        [
            "budget.toml:2: `Cargo.toml` has 2 lints set to `allow`, over its budget of 1: remove \
             one, or raise `Cargo.toml` under `[allow]` in budget.toml, which the owner reviews \
             (budget)",
            "budget.toml:3: `gone/Cargo.toml` is no manifest of the repository: remove it from \
             `[allow]` (budget)",
        ]
    );
}

#[test]
fn a_deviation_scoped_to_a_crate_excuses_its_budget() {
    let fixture = Fixture::new();
    fixture
        .add_crate("listed", INHERITS)
        .add_crate("unlisted", INHERITS)
        .write(
            "crates/listed/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n",
        )
        .write(
            "crates/unlisted/src/lib.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n",
        )
        .write("budget.toml", "[expect]\nlisted = 3\n")
        .write(
            "standard.toml",
            "[deviations]\nbudget = { reason = \"r\", crates = [\"listed\", \"unlisted\"] }\n",
        );
    assert_eq!(fixture.violations(Rule::Budget), [] as [&str; 0]);
}

#[test]
fn another_workspaces_virtual_root_has_an_allow_budget() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "other/Cargo.toml",
            "[workspace]\nmembers = [\"member\"]\nresolver = \"3\"\n\n\
             [workspace.lints.clippy]\npedantic = \"allow\"\n",
        )
        .write(
            "other/member/Cargo.toml",
            "[package]\nname = \"member\"\nedition = \"2024\"\n",
        )
        .write("other/member/src/lib.rs", "");
    std::process::Command::new("cargo")
        .args([
            "generate-lockfile",
            "--offline",
            "--manifest-path",
            "other/Cargo.toml",
        ])
        .current_dir(fixture.0.root())
        .status()
        .expect("cargo locks the other workspace");
    assert_eq!(
        fixture.violations(Rule::Budget),
        [
            "other/Cargo.toml:1: `other/Cargo.toml` has 1 lints set to `allow`, over its budget of \
          0: remove one, or raise `other/Cargo.toml` under `[allow]` in budget.toml, which the \
          owner reviews (budget)"
        ]
    );
}

#[test]
fn a_file_a_cfg_attr_path_or_a_macro_reaches_counts() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "crates/core/src/lib.rs",
            "#[cfg_attr(unix, path = \"unix.rs\")]\nmod sys;\nmacro_rules! m {\n    () => {\n        \
             mod hidden;\n    };\n}\nm!();\n",
        )
        .write(
            "crates/core/src/unix.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn f() {}\n",
        )
        .write(
            "crates/core/src/hidden.rs",
            "#[expect(dead_code, reason = \"r\")]\nfn g() {}\n",
        )
        .write("budget.toml", "[expect]\ncore = 2\n");
    assert_eq!(fixture.violations(Rule::Budget), [] as [&str; 0]);
}
