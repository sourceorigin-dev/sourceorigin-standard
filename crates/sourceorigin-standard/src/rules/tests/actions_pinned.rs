use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn actions_are_pinned_to_a_commit_with_their_version() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        ".github/workflows/ci.yml",
        "jobs:\n  rust:\n    steps:\n      \
         - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n      \
         - uses: actions/cache@v4\n      \
         - uses: Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6\n      \
         - uses: ./.github/actions/ui\n  \
         release:\n    uses: owner/repo/.github/workflows/release.yml@main\n",
    );
    assert_eq!(
        fixture.violations(Rule::ActionsPinned),
        [
            ".github/workflows/ci.yml:5: `actions/cache@v4` isn't pinned: name the commit of its \
             release, `actions/cache@<40-character SHA> # v<version>` (actions-pinned)",
            ".github/workflows/ci.yml:6: `Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6` \
             isn't pinned: name the commit of its release, `Swatinem/rust-cache@<40-character \
             SHA> # v<version>` (actions-pinned)",
            ".github/workflows/ci.yml:9: `owner/repo/.github/workflows/release.yml@main` isn't \
             pinned: name the commit of its release, \
             `owner/repo/.github/workflows/release.yml@<40-character SHA> # v<version>` \
             (actions-pinned)",
        ]
    );
}
