use std::collections::BTreeMap;

use super::{Fixture, INHERITS, LINTS};
use crate::rules::report;

#[test]
fn the_report_holds_violations_deviations_and_exceptions() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .add_crate("bare", "")
        .write(
            "Cargo.toml",
            &format!(
                "[workspace]\nmembers = [\"crates/*\"]\n\n{LINTS}module_name_repetitions = \
                 \"allow\"\n"
            ),
        )
        .write(
            "crates/core/src/lib.rs",
            "#[expect(dead_code, clippy::panic, reason = \"r\")]\nfn f() {\n    \
             // INVARIANT: one.\n    let a = 0;\n}\n#[expect(dead_code, reason = \"r\")]\nfn g() {}\n",
        )
        .write(
            "budget.toml",
            "[expect]\ncore = 3\n\n[invariant]\ncore = 1\n\n[allow]\n\"Cargo.toml\" = 1\n",
        )
        .write(
            "standard.toml",
            "[deviations]\nworkspace-lints = \"a crate on its own\"\n",
        );
    let report = report(fixture.0.root()).expect("the report is made");
    let lints = &report.rules["workspace-lints"];
    assert_eq!(lints.deviation.as_deref(), Some("a crate on its own"));
    assert_eq!(
        lints
            .violations
            .iter()
            .map(|violation| violation.path.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        ["crates/bare/Cargo.toml"]
    );
    assert_eq!(report.rules["budget"].violations, []);
    assert_eq!(
        report.expects,
        BTreeMap::from([("clippy::panic".to_owned(), 1), ("dead_code".to_owned(), 2)])
    );
    assert_eq!(report.invariants, 1);
    assert_eq!(report.workspace_allows, ["clippy::module_name_repetitions"]);
    let json = report.to_json().expect("the report prints");
    assert!(json.contains("\"rule\": \"workspace-lints\""), "{json}");
}
