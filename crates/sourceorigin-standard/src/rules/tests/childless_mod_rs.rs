use super::{Fixture, INHERITS};
use crate::rules::{Rule, fix};

/// The violation of the childless `crates/core/src/{module}/mod.rs`, with
/// `withheld` saying why `--fix` leaves it.
fn childless(module: &str, withheld: &str) -> String {
    let path = format!("crates/core/src/{module}/mod.rs");
    format!(
        "{path}:1: `{path}` has no child module: move it to `crates/core/src/{module}.rs`{} \
         (childless-mod-rs)",
        if withheld.is_empty() {
            String::new()
        } else {
            format!(" (by hand: `--fix` doesn't, as {withheld})")
        }
    )
}

#[test]
fn a_mod_rs_without_children_moves_to_its_module_name() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "crates/core/src/lib.rs",
            "mod broken;\nmod db;\nmod leaf;\nmod parent;\nmod styled;\n",
        )
        .write("crates/core/src/leaf/mod.rs", "pub fn f() {}\n")
        .write("crates/core/src/parent/mod.rs", "mod nested;\n")
        .write("crates/core/src/parent/nested/mod.rs", "")
        .write("crates/core/src/broken/mod.rs", "mod child;\n")
        .write("crates/core/src/broken/child.rs", "fn {\n")
        .write("crates/core/src/lone/mod.rs", "fn {\n")
        .write(
            "crates/core/src/db/mod.rs",
            "const SCHEMA: &str = include_str!(\"../schema.sql\");\n",
        )
        .write("crates/core/src/styled/mod.rs", "")
        .write("crates/core/src/styled/theme.css", "")
        .write("crates/core/src/bin/tool.rs", "mod common;\nfn main() {}\n")
        .write("crates/core/src/bin/common/mod.rs", "")
        .write("crates/core/tests/it.rs", "mod common;\n")
        .write("crates/core/tests/common/mod.rs", "");
    assert_eq!(
        fixture.violations(Rule::ChildlessModRs),
        [
            childless("db", "it may name a file by a relative path"),
            childless("leaf", ""),
            childless("lone", "it may name a file by a relative path"),
            childless("parent/nested", ""),
            childless("styled", "its directory holds other files"),
        ]
    );

    fixture.write(
        "standard.toml",
        "[deviations]\nchildless-mod-rs = { reason = \"generated\", paths = \
         [\"crates/core/src/parent/**\"] }\n",
    );
    fix(fixture.0.root()).expect("the fixes are made");
    let root = fixture.0.root().join("crates/core/src");
    assert_eq!(
        std::fs::read_to_string(root.join("leaf.rs")).expect("the module moved"),
        "pub fn f() {}\n"
    );
    assert!(!root.join("leaf").exists(), "its directory is removed");
    for kept in [
        "parent/nested/mod.rs",
        "db/mod.rs",
        "lone/mod.rs",
        "styled/mod.rs",
    ] {
        assert!(root.join(kept).exists(), "{kept} stays");
    }
}

#[test]
fn a_mod_rs_a_path_or_manifest_names_is_moved_by_hand() {
    let fixture = Fixture::new();
    fixture
        .add_crate(
            "core",
            &format!("{INHERITS}\n[[test]]\nname = \"it\"\npath = \"tests/it/mod.rs\"\n"),
        )
        .write(
            "crates/core/src/lib.rs",
            "#[cfg(unix)]\n#[path = \"imp/mod.rs\"]\nmod sys;\n",
        )
        .write("crates/core/src/imp/mod.rs", "pub fn f() {}\n")
        .write("crates/core/tests/it/mod.rs", "#[test]\nfn t() {}\n");
    let path = |file: &str| format!("crates/core/{file}/mod.rs");
    assert_eq!(
        fixture.violations(Rule::ChildlessModRs),
        [
            format!(
                "{}:1: `{}` has no child module: move it to `crates/core/src/imp.rs` (by hand: \
                 `--fix` doesn't, as a `#[path]` attribute names it) (childless-mod-rs)",
                path("src/imp"),
                path("src/imp")
            ),
            format!(
                "{}:1: `{}` has no child module: move it to `crates/core/tests/it.rs` (by hand: \
                 `--fix` doesn't, as it is a target's root, which its manifest names) \
                 (childless-mod-rs)",
                path("tests/it"),
                path("tests/it")
            ),
        ]
    );
    assert!(
        fix(fixture.0.root())
            .expect("the fixes are made")
            .is_empty()
    );
}

#[test]
fn a_mod_rs_a_cfg_attr_path_names_is_moved_by_hand() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "crates/core/src/lib.rs",
            "#[cfg_attr(unix, path = \"unix/mod.rs\")]\nmod sys;\n",
        )
        .write("crates/core/src/unix/mod.rs", "pub fn f() {}\n");
    assert!(
        fix(fixture.0.root())
            .expect("the fixes are made")
            .is_empty()
    );
    assert!(
        fixture
            .0
            .root()
            .join("crates/core/src/unix/mod.rs")
            .exists()
    );
}

#[test]
fn a_mod_rs_whose_cfg_attr_path_names_a_file_is_moved_by_hand() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("crates/core/src/lib.rs", "mod os;\n")
        .write(
            "crates/core/src/os/mod.rs",
            "#[cfg_attr(unix, path = \"../unix.rs\")]\nmod sys;\n",
        )
        .write("crates/core/src/unix.rs", "pub fn f() {}\n");
    assert!(
        fix(fixture.0.root())
            .expect("the fixes are made")
            .is_empty()
    );
    assert!(fixture.0.root().join("crates/core/src/os/mod.rs").exists());
}

#[test]
fn a_mod_rs_another_workspaces_path_names_is_moved_by_hand() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "lints/Cargo.toml",
            "[package]\nname = \"lints\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .write("lints/src/lib.rs", "#[path = \"imp/mod.rs\"]\nmod sys;\n")
        .write("lints/src/imp/mod.rs", "pub fn f() {}\n");
    std::process::Command::new("cargo")
        .args([
            "generate-lockfile",
            "--offline",
            "--manifest-path",
            "lints/Cargo.toml",
        ])
        .current_dir(fixture.0.root())
        .status()
        .expect("cargo locks the other workspace");
    assert!(
        fix(fixture.0.root())
            .expect("the fixes are made")
            .is_empty()
    );
    assert!(fixture.0.root().join("lints/src/imp/mod.rs").exists());
}
