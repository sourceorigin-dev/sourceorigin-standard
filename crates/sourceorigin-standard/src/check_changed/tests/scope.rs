use std::path::PathBuf;

use super::super::change::{Crate, Scope};
use super::paths;
use crate::config::Builds;
use crate::metadata::{Dependency, Metadata, Package, Target, TargetKind};

/// A crate in `/w/<dir>`, depending on `dependencies`, with a library or a
/// binary at `<dir>/src`.
fn package(dir: &str, dependencies: &[&str], library: bool) -> Package {
    let name = if dir.is_empty() {
        "root"
    } else {
        dir.rsplit('/').next().expect("a name")
    };
    let source = if library { "lib.rs" } else { "main.rs" };
    Package {
        name: name.to_owned(),
        manifest_path: PathBuf::from(format!("/w/{dir}/Cargo.toml")),
        dependencies: dependencies
            .iter()
            .map(|name| Dependency {
                name: (*name).to_owned(),
            })
            .collect(),
        targets: vec![Target {
            name: name.to_owned(),
            kind: vec![if library {
                TargetKind::Lib
            } else {
                TargetKind::Bin
            }],
            src_path: PathBuf::from(format!("/w/{dir}/src/{source}")),
        }],
    }
}

/// A workspace of these crates, rooted at `/w`.
fn metadata(packages: Vec<Package>) -> Metadata {
    Metadata {
        packages,
        workspace_root: PathBuf::from("/w"),
    }
}

/// A workspace whose daemon embeds its dashboard.
fn piqueld() -> Metadata {
    metadata(vec![
        package("crates/core", &["serde"], true),
        package("crates/client", &["core"], true),
        package("apps/cli", &["client"], true),
        package("apps/ui", &["client"], true),
        package("apps/daemon", &["core"], true),
        package("tools/xtask", &["client"], false),
    ])
}

/// The daemon embeds the dashboard.
fn builds() -> [Builds; 1] {
    [Builds {
        builder: "daemon".to_owned(),
        built: "ui".to_owned(),
    }]
}

fn crates(names: &[(&str, bool)]) -> Scope {
    Scope::Crates(
        names
            .iter()
            .map(|&(name, library)| Crate {
                name: name.to_owned(),
                library,
            })
            .collect(),
    )
}

#[test]
fn a_change_reaches_its_crates_dependents() {
    let changed = paths(&["crates/client/src/lib.rs"]);
    assert_eq!(
        Scope::new(&piqueld(), &builds(), &changed),
        crates(&[
            ("client", true),
            ("cli", true),
            ("ui", true),
            ("daemon", true),
            ("xtask", false),
        ])
    );
}

#[test]
fn a_change_reaches_the_crate_building_it() {
    let changed = paths(&["apps/ui/src/app.rs"]);
    assert_eq!(
        Scope::new(&piqueld(), &builds(), &changed),
        crates(&[("ui", true), ("daemon", true)])
    );
}

#[test]
fn a_path_in_no_crate_reaches_the_workspace() {
    let changed = paths(&["apps/cli/src/main.rs", "Cargo.lock"]);
    assert_eq!(
        Scope::new(&piqueld(), &builds(), &changed),
        Scope::Workspace {
            path: "Cargo.lock".to_owned()
        }
    );
}

/// As DirkEngine's binary, whose directory is the workspace's.
#[test]
fn a_crate_at_the_root_owns_only_its_targets_entries() {
    let workspace = metadata(vec![
        package("", &["engine"], false),
        package("crates/engine", &[], true),
    ]);
    assert_eq!(
        Scope::new(&workspace, &[], &paths(&["src/main.rs"])),
        crates(&[("root", false)])
    );
    assert_eq!(
        Scope::new(&workspace, &[], &paths(&["crates/engine/src/lib.rs"])),
        crates(&[("root", false), ("engine", true)])
    );
    assert_eq!(
        Scope::new(&workspace, &[], &paths(&["rust-toolchain.toml"])),
        Scope::Workspace {
            path: "rust-toolchain.toml".to_owned()
        }
    );
}
