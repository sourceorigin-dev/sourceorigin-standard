use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn cargo_deny_denies_unknown_sources_and_wildcards_and_explains_ignores() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        "deny.toml",
        "[advisories]\nignore = [\n    \"RUSTSEC-2026-0001\",\n    \
         { id = \"RUSTSEC-2026-0002\", reason = \"unreachable\" },\n]\n\n[licenses]\nallow = []\n\n\
         [bans]\nwildcards = \"warn\"\n\n[sources]\nunknown-registry = \"deny\"\n",
    );
    assert_eq!(
        fixture.violations(Rule::DenyConfig),
        [
            "deny.toml:3: the ignored advisory `RUSTSEC-2026-0001` gives no reason: write it as \
             `{ id = \"RUSTSEC-2026-0001\", reason = \"<why it doesn't apply, and what ends the \
             ignore>\" }` (deny-config)",
            "deny.toml:7: `[licenses]` allows no license: list the licenses the repository \
             accepts in `allow` (deny-config)",
            "deny.toml:11: `[bans]` doesn't deny `wildcards`: set `wildcards = \"deny\"` \
             (deny-config)",
            "deny.toml:13: `[sources]` doesn't deny `unknown-git`: set `unknown-git = \"deny\"` \
             (deny-config)",
        ]
    );
    fixture.fix_rule(Rule::DenyConfig);
    assert_eq!(fixture.violations(Rule::DenyConfig).len(), 2);
}
