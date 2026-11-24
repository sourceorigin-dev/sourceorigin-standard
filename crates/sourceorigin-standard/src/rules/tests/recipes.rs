use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A justfile with every required recipe, `default` and `validate` as the
/// standard has them.
const JUSTFILE: &str = "\
default: check-changed

validate: fmt lint standard test doc doc-test audit unused

fmt:
fmt-check:
lint:
standard:
[positional-arguments]
test *ARGS='--workspace':
    cargo nextest run --locked \"$@\"
doc:
doc-test:
audit:
unused:
check-changed:
";

#[test]
fn the_justfile_has_the_standard_recipes() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("justfile", JUSTFILE);
    assert_eq!(fixture.violations(Rule::Recipes), Vec::<String>::new());

    fixture.write(
        "justfile",
        "build:\n\nvalidate: lint fmt-check test\n\nfmt:\nfmt-check:\nlint:\ntest:\n",
    );
    let violations: Vec<String> = [
        "no `standard` recipe: add it",
        "no `doc` recipe: add it",
        "no `doc-test` recipe: add it",
        "no `audit` recipe: add it",
        "no `unused` recipe: add it",
        "no `check-changed` recipe: add it",
        "`just` alone runs `build`: make the first recipe `default: validate` or \
         `default: check-changed`",
        "`validate` doesn't format first: make `fmt` its first dependency",
        "`validate` doesn't run `standard`: add it to its dependencies",
        "`validate` doesn't run `doc`: add it to its dependencies",
        "`validate` doesn't run `doc-test`: add it to its dependencies",
        "`validate` doesn't run `audit`: add it to its dependencies",
        "`validate` doesn't run `unused`: add it to its dependencies",
        "`validate` runs `fmt-check` after formatting: remove it",
        "`test` doesn't pass arguments on: give it `*ARGS='--workspace'`",
    ]
    .iter()
    .map(|message| format!("justfile:1: {message} (recipes)"))
    .collect();
    assert_eq!(fixture.violations(Rule::Recipes), violations);
}
