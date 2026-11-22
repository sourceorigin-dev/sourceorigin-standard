use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// Steps saving caches from `main` only, and steps saving them anywhere.
const STEPS: &str = "\
jobs:
  rust:
    steps:
      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2
        with:
          save-if: ${{ github.ref == 'refs/heads/main' }}
      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2
      - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6 # v2
        with:
          save-if: false
      - uses: actions/cache/restore@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
      - uses: actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9 # v6.1.0
";

#[test]
fn caches_are_saved_from_main_only() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            ".github/workflows/ci.yml",
            &format!("on: [pull_request]\n{STEPS}"),
        )
        .write(
            ".github/workflows/main.yml",
            &format!("on:\n  push:\n{STEPS}"),
        );
    let rust_cache = |path: &str| {
        format!(
            "{path}:8: the Rust cache is saved off `main`: set `save-if: ${{{{ github.ref == \
             'refs/heads/main' }}}}` (cache-saves)"
        )
    };
    assert_eq!(
        fixture.violations(Rule::CacheSaves),
        [
            rust_cache(".github/workflows/ci.yml"),
            ".github/workflows/ci.yml:13: `actions/cache` saves on a miss, here off `main`: \
             restore with `actions/cache/restore`, and save with `actions/cache/save` from \
             `main` only (cache-saves)"
                .to_owned(),
            rust_cache(".github/workflows/main.yml").replace(":8:", ":9:"),
        ]
    );
}
