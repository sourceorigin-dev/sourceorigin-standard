use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A crate root with `Duration`s in every scope; lines 11, 12, 15 and 22
/// hold inline ones.
const LIB: &str = r#"use std::time::Duration;
mod support;
#[cfg(test)]
mod tests;
/// Named.
const TIMEOUT: Duration = Duration::from_secs(5);
static TICK: Duration = Duration::from_millis(10);
const fn limit() -> Duration { Duration::from_secs(1) }
fn wait(timeout: Duration) -> Duration { timeout }
fn run() {
    wait(Duration::from_secs(30));
    wait(Duration::from_secs(2 * 60));
    wait(TIMEOUT);
    wait(Duration::from_secs(u64::from(limit_secs())));
    println!("{:?}", std::time::Duration::from_millis(5));
}
#[tokio::test]
async fn waits() { wait(Duration::from_secs(1)); }
#[cfg(feature = "test-util")]
fn fixture() -> Duration { Duration::from_secs(3) }
#[cfg(feature = "attestation")]
fn attest() -> Duration { Duration::from_secs(4) }
#[cfg(feature = "integration-tests")]
fn integration() -> Duration { Duration::from_secs(4) }
"#;

/// A `Duration` with a literal.
const INLINE: &str = "fn f() { let _ = std::time::Duration::from_secs(1); }\n";

#[test]
fn durations_with_literals_sit_in_constants_or_test_code() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("crates/core/src/lib.rs", LIB)
        .write("crates/core/src/tests.rs", INLINE)
        .write(
            "crates/core/src/support.rs",
            "#[cfg(any(test, feature = \"test-util\"))]\nmod helpers;\n",
        )
        .write("crates/core/src/support/helpers.rs", INLINE)
        .write("crates/core/tests/it.rs", INLINE);
    let message = |line: usize, constructor: &str| {
        format!(
            "crates/core/src/lib.rs:{line}: `Duration::{constructor}` with a literal is an \
             inline limit: make it a named `const` with a doc comment (inline-limits)"
        )
    };
    assert_eq!(
        fixture.violations(Rule::InlineLimits),
        [
            message(11, "from_secs"),
            message(12, "from_secs"),
            message(15, "from_millis"),
            message(22, "from_secs"),
        ]
    );
}
