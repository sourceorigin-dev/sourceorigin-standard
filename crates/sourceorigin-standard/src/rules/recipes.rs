//! `recipes`: every repository's justfile runs the same checks under the
//! same names.

use std::path::Path;

use super::justfile::{self, Justfile};
use super::{Finding, Repository};

/// The recipes every justfile has.
const REQUIRED: [&str; 11] = [
    "validate",
    "fmt",
    "fmt-check",
    "lint",
    "standard",
    "test",
    "doc",
    "doc-test",
    "audit",
    "unused",
    "check-changed",
];

/// What `validate` runs first: `fmt`, or `fmt-check` for a `validate` that
/// changes no files.
const FORMATS: [&str; 2] = ["fmt", "fmt-check"];

/// The recipes `validate` runs, after one of [`FORMATS`].
const VALIDATED: [&str; 7] = [
    "lint", "standard", "test", "doc", "doc-test", "audit", "unused",
];

/// What `just` alone may run.
const DEFAULTS: [&str; 2] = ["validate", "check-changed"];

/// The default `test` passes on.
const TEST_DEFAULT: &str = "--workspace";

impl Justfile {
    /// Where the justfile breaks the standard, each with what fixes it.
    fn violations(&self) -> Vec<String> {
        let mut violations: Vec<String> = REQUIRED
            .iter()
            .filter(|&&name| !self.recipes.contains_key(name))
            .map(|name| format!("no `{name}` recipe: add it"))
            .collect();
        let first = self.first.as_deref().unwrap_or_default();
        let bodiless = self
            .recipes
            .get(first)
            .is_some_and(|recipe| recipe.lines().next().is_none());
        let runs = match self.dependencies(first).as_slice() {
            [only] if bodiless => *only,
            _ => first,
        };
        if !DEFAULTS.contains(&runs) {
            violations.push(format!(
                "`just` alone runs `{first}`: make the first recipe `default: validate` or \
                 `default: check-changed`"
            ));
        }
        if self.recipes.contains_key("validate") {
            let validated = self.dependencies("validate");
            if !validated
                .first()
                .is_some_and(|first| FORMATS.contains(first))
            {
                violations.push(
                    "`validate` doesn't format first: make `fmt` its first dependency, or \
                     `fmt-check` for a `validate` that changes no files"
                        .to_owned(),
                );
            }
            violations.extend(
                VALIDATED
                    .iter()
                    .filter(|name| !validated.contains(name))
                    .map(|name| {
                        format!("`validate` doesn't run `{name}`: add it to its dependencies")
                    }),
            );
            if FORMATS.iter().all(|format| validated.contains(format)) {
                violations
                    .push("`validate` runs `fmt-check` after formatting: remove it".to_owned());
            }
        }
        if let Some(test) = self.recipes.get("test") {
            let passes_on = test.parameters.iter().any(|parameter| {
                parameter.kind == "star"
                    && parameter
                        .default
                        .as_ref()
                        .and_then(serde_json::Value::as_str)
                        == Some(TEST_DEFAULT)
            });
            if !passes_on {
                violations.push(format!(
                    "`test` doesn't pass arguments on: give it `*ARGS='{TEST_DEFAULT}'`"
                ));
            }
        }
        violations
    }
}

impl Repository {
    /// Each way the justfile breaks the standard, or a missing justfile;
    /// one `just` can't read is `config-syntax`'s.
    pub(super) fn recipes(&self) -> Vec<Finding> {
        let exists = self.unreadable.contains(justfile::PATH)
            || self
                .justfiles
                .iter()
                .any(|file| file.path == Path::new(justfile::PATH));
        let violations = match &self.justfile {
            Some(justfile) => justfile.violations(),
            None if exists => Vec::new(),
            None => vec!["no justfile: add one with the standard's recipes".to_owned()],
        };
        violations
            .into_iter()
            .map(|message| Finding {
                path: justfile::PATH.into(),
                line: 1,
                message,
            })
            .collect()
    }
}
