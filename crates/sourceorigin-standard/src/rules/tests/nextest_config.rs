use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn hung_tests_fail_and_flaky_ones_are_not_retried() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        ".config/nextest.toml",
        "[profile.default]\nslow-timeout = { period = \"60s\", terminate-after = 5 }\n\n\
         [[profile.default.overrides]]\nfilter = \"binary(docker)\"\nslow-timeout = \"60s\"\n\n\
         [profile.ci]\nretries = 2\n",
    );
    assert_eq!(
        fixture.violations(Rule::NextestConfig),
        [
            ".config/nextest.toml:6: a `slow-timeout` without `terminate-after` lets a hung test \
             run forever: add `terminate-after` (nextest-config)",
            ".config/nextest.toml:9: `retries` hides flaky tests: remove it, and fix the test \
             (nextest-config)",
        ]
    );
}

#[test]
fn the_default_profile_sets_a_slow_timeout() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS);
    assert_eq!(
        fixture.violations(Rule::NextestConfig),
        [
            ".config/nextest.toml:1: `[profile.default]` sets no `slow-timeout`: add \
          `slow-timeout = { period = \"60s\", terminate-after = 5 }` (nextest-config)"
        ]
    );
}
