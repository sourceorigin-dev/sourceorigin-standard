use super::{Fixture, INHERITS};
use crate::rules::Rule;

#[test]
fn the_pull_request_template_asks_what_why_and_how_it_was_tested() {
    let fixture = Fixture::new();
    fixture.add_crate("core", INHERITS).write(
        ".github/pull_request_template.md",
        "## What and why\n\n<!-- What changes. -->\n\n## Checklist\n",
    );
    assert_eq!(
        fixture.violations(Rule::PrTemplate),
        [
            ".github/pull_request_template.md:1: the pull request template has no `## Testing`: \
          add it (pr-template)"
        ]
    );
    fixture.fix_rule(Rule::PrTemplate);
    assert_eq!(fixture.violations(Rule::PrTemplate), Vec::<String>::new());
}
