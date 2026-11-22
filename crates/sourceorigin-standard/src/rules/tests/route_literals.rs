use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A router and redirects, with literal paths on lines 3, 4 and 6.
const APP: &str = r#"fn router() -> Router {
    Router::new()
        .route("/settings", get(settings))
        .nest("/api", api())
        .route(routes::HOME, get(home));
    let _ = Redirect::to("/signed-out");
    let _ = Redirect::to("https://example.com/");
}
#[test]
fn routes() {
    Router::new().route("/test", get(ok));
}
"#;

#[test]
fn only_the_listed_route_modules_write_path_literals() {
    let fixture = Fixture::new();
    fixture
        .add_crate("app", INHERITS)
        .write("crates/app/src/lib.rs", "mod handlers;\nmod routes;\n")
        .write("crates/app/src/handlers.rs", APP)
        .write(
            "crates/app/src/routes.rs",
            "mod admin;\nfn r() { Router::new().route(\"/\", get(home)); }\n",
        )
        .write(
            "crates/app/src/routes/admin.rs",
            "fn r() { Router::new().nest(\"/admin\", admin()); }\n",
        )
        .write(
            "crates/app/examples/demo.rs",
            "fn main() { Router::new().route(\"/\", get(home)); }\n",
        )
        .write(
            "standard.toml",
            "route-modules = [\n    \"app::routes\",\n    \"app::paths\",\n]\n",
        );
    let message = |line: usize, what: &str, path: &str| {
        format!(
            "crates/app/src/handlers.rs:{line}: `{what}` takes the path `{path}` as a literal: \
             name it in a module `route-modules` in standard.toml lists, and use that \
             (route-literals)"
        )
    };
    assert_eq!(
        fixture.violations(Rule::RouteLiterals),
        [
            message(3, ".route", "/settings"),
            message(4, ".nest", "/api"),
            message(6, "Redirect::to", "/signed-out"),
            "standard.toml:3: `app::paths` is in `route-modules`, but no file is that module: \
             remove it (route-literals)"
                .to_owned(),
        ]
    );
}

#[test]
fn without_route_modules_paths_are_not_checked() {
    let fixture = Fixture::new();
    fixture
        .add_crate("app", INHERITS)
        .write("crates/app/src/lib.rs", APP);
    assert_eq!(
        fixture.violations(Rule::RouteLiterals),
        Vec::<String>::new()
    );
}
