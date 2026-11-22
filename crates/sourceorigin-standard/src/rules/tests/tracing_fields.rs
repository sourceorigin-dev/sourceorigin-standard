use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// Events, spans and `#[instrument]` functions, recording denied fields on
/// lines 2, 3, 6 to 10, 12 and 14.
const LIB: &str = r#"fn serve(token: &str, request: &Request) {
    tracing::info!(token = %token, "signed in");
    info!(?request.path, "request");
    tracing::debug!("token {}", token);
    let _span = info_span!("handle", user = %request.user);
    warn!(target: "audit", %token);
    let _span = info_span!("handle", token = %token);
    let _span = span!(Level::INFO, "handle", token = 1);
    info!("user_agent" = %agent, token = 1);
    let _span = info_span!("db", db.query.text = sql, url.query = query);
}
#[instrument]
fn sign_in(token: &str, user: &str) {}
#[instrument(skip(token), fields(path = %request.path))]
fn sign_out(token: &str, request: &Request) {}
#[instrument(skip_all)]
fn quiet(token: &str) {}
#[test]
fn logs() {
    info!(token = 1);
}
"#;

#[test]
fn events_spans_and_instrumented_functions_record_no_denied_field() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("crates/core/src/lib.rs", LIB)
        .write(
            "standard.toml",
            "denied-fields = [\"token\", \"path\", \"user_agent\", \"url.query\"]\n",
        );
    let message = |line: usize, what: &str, field: &str, denied: &str| {
        format!(
            "crates/core/src/lib.rs:{line}: {what} `{field}`, and telemetry never holds a \
             `{denied}` (`denied-fields` in standard.toml): leave it out, or skip it \
             (tracing-fields)"
        )
    };
    assert_eq!(
        fixture.violations(Rule::TracingFields),
        [
            message(2, "`info!` records the field", "token", "token"),
            message(3, "`info!` records the field", "request.path", "path"),
            message(6, "`warn!` records the field", "token", "token"),
            message(7, "`info_span!` records the field", "token", "token"),
            message(8, "`span!` records the field", "token", "token"),
            message(9, "`info!` records the field", "user_agent", "user_agent"),
            message(9, "`info!` records the field", "token", "token"),
            message(
                10,
                "`info_span!` records the field",
                "url.query",
                "url.query"
            ),
            message(12, "`#[instrument]` records the argument", "token", "token"),
            message(14, "`#[instrument]` records the field", "path", "path"),
        ]
    );

    fixture.write("standard.toml", "");
    assert_eq!(
        fixture.violations(Rule::TracingFields),
        Vec::<String>::new()
    );
}

#[test]
fn a_destructured_argument_counts_and_another_crates_macro_doesnt() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("standard.toml", "denied-fields = [\"token\"]\n")
        .write(
            "crates/core/src/lib.rs",
            "#[instrument]\nfn pair((token, id): (&str, u8)) {}\nfn logs(token: &str) {\n    \
             log::info!(token = token);\n}\n",
        );
    assert_eq!(
        fixture.violations(Rule::TracingFields),
        [
            "crates/core/src/lib.rs:1: `#[instrument]` records the argument `token`, and telemetry \
          never holds a `token` (`denied-fields` in standard.toml): leave it out, or skip it \
          (tracing-fields)"
        ]
    );
}
