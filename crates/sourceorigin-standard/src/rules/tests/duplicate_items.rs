use super::{Fixture, INHERITS};
use crate::rules::Rule;

/// A function whose body holds 50 tokens.
const TOTAL: &str = "fn total(items: &[u32]) -> u32 {
    let limit = items.len() as u32;
    let mut total = 0;
    for item in items {
        if *item > 10 {
            total += item * 2;
        } else {
            total += item + 1;
        }
    }
    total + limit
}
";

/// `TOTAL` with every name changed.
const SUM: &str = "fn sum(values: &[u32]) -> u32 {
    let count = values.len() as u32;
    let mut sum = 0;
    for value in values {
        if *value > 10 {
            sum += value * 2;
        } else {
            sum += value + 1;
        }
    }
    sum + count
}
";

/// A function of fewer than 20 tokens, twice.
const SMALL: &str =
    "fn a(x: u32) -> u32 {\n    x * 2 + 1\n}\nfn b(y: u32) -> u32 {\n    y * 2 + 1\n}\n";

#[test]
fn a_body_of_20_tokens_is_written_once() {
    let measure = TOTAL
        .replace("total(", "measure(")
        .replace("items.len()", "items.count()");
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write(
            "crates/core/src/lib.rs",
            &format!("mod other;\n{TOTAL}{SMALL}{measure}"),
        )
        .write("crates/core/src/other.rs", SUM)
        .write(
            "crates/core/tests/it.rs",
            &format!(
                "{}{}",
                TOTAL.replace("total(", "copy("),
                SUM.replace("sum(", "again(")
            ),
        );
    assert_eq!(
        fixture.violations(Rule::DuplicateItems),
        [
            "crates/core/src/other.rs:1: the body of `sum` repeats that of `total` \
          (crates/core/src/lib.rs:2), names aside: write it once, on the type that owns it, \
          and call it (duplicate-items)"
        ]
    );
}

/// Bodies alike but for the fields they name, or for `&` and `&mut`.
const FIELDS: &str = "pub struct A { code: u32, path: String, message: String }
pub struct B { code: u32, resource: String, message: String }
impl A {
    pub fn new(code: u32, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), path: path.into(), message: message.into() }
    }
}
impl B {
    pub fn new(code: u32, resource: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), resource: resource.into(), message: message.into() }
    }
}
pub enum C { D { depth: u32, stencil: u32 } }
impl C {
    pub fn depth(&self) -> Option<u32> {
        match self { Self::D { depth, .. } if *depth > 0 => Some(*depth), Self::D { .. } => None }
    }
    pub fn stencil(&self) -> Option<u32> {
        match self { Self::D { stencil, .. } if *stencil > 0 => Some(*stencil), Self::D { .. } => None }
    }
    pub fn first(&self) -> Option<&u32> {
        match self { Self::D { depth, stencil } if *depth > *stencil => Some(depth), Self::D { .. } => None }
    }
    pub fn first_mut(&mut self) -> Option<&mut u32> {
        match self { Self::D { depth, stencil } if *depth > *stencil => Some(depth), Self::D { .. } => None }
    }
}
";

#[test]
fn the_fields_a_body_names_and_its_references_mutability_are_its_shape() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("crates/core/src/lib.rs", FIELDS);
    assert_eq!(
        fixture.violations(Rule::DuplicateItems),
        Vec::<String>::new()
    );
}
