use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn every_page_of_the_book_is_reachable() {
    let fixture = Fixture::new();
    fixture
        .add_crate("core", INHERITS)
        .write("book.toml", "[book]\ntitle = \"Docs\"\nsrc = \"docs\"\n")
        .write(
            "docs/SUMMARY.md",
            "# Summary\n\n- [Guide](./user/guide.md)\n  - [Setup](user/setup.md#install)\n",
        )
        .write(
            "docs/user/guide.md",
            "# Guide\n\n{{#include ../reference/flags.md:2:}}\n",
        )
        .write("docs/user/setup.md", "# Setup\n")
        .write("docs/reference/flags.md", "- `--flag`\n")
        .write("docs/dev/forgotten.md", "# Forgotten\n")
        .write("crates/core/README.md", "# Core\n");
    assert_eq!(
        fixture.violations(Rule::DocsSummary),
        [
            "docs/dev/forgotten.md:1: no page reaches this one: link it from docs/SUMMARY.md, or \
          include it in a page with `{{#include …}}` (docs-summary)"
        ]
    );
}
