use std::collections::BTreeSet;

use super::super::filters::Quantifier;
use super::super::workflow::Workflow;
use super::super::{Drift, Error};
use super::CI;

/// A workflow matching [`CI`].
const WORKFLOW: &str = "
on: pull_request
jobs:
  changes:
    outputs:
      rust: ${{ github.event_name != 'pull_request' || steps.filter.outputs.rust == 'true' }}
      deny: ${{ github.event_name != 'pull_request' || steps.filter.outputs.deny == 'true' }}
      ui: ${{ steps.filter.outputs.ui == 'true' && !github.event.pull_request.draft }}
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
      - id: filter
        uses: dorny/paths-filter@ceb8a2b8f2d89434be7ff52d3de7ec3738c5cc9d # v4.0.3
        with:
          filters: .github/filters.yml
  checks:
    needs: changes
    if: needs.changes.outputs.rust == 'true' || needs.changes.outputs.deny == 'true'
  rust:
    needs: changes
    if: needs.changes.outputs.rust == 'true'
  ui:
    needs: changes
    if: needs.changes.outputs.ui == 'true'
  docs:
    runs-on: ubuntu-latest
  ci:
    if: always()
    needs: [changes, checks, rust, ui, docs]
";

fn names(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(ToString::to_string).collect()
}

#[test]
fn a_table_matching_the_workflow_has_no_drift() {
    let workflow = Workflow::parse(WORKFLOW).expect("the workflow parses");
    assert_eq!(workflow.quantifier, Quantifier::Some);
    assert_eq!(CI.drifts(&workflow), []);
}

#[test]
fn drift_between_the_table_and_the_workflow_is_named() {
    let cases = [
        (
            "  ui:\n    needs: changes\n    if: needs.changes.outputs.ui == 'true'\n",
            "",
            vec![Drift::Missing { job: "ui" }],
        ),
        (
            "  docs:\n",
            "  docker:\n    if: needs.changes.outputs.rust == 'true'\n  docs:\n",
            vec![Drift::Unlisted {
                job: "docker".to_owned(),
            }],
        ),
        (
            "    if: needs.changes.outputs.rust == 'true' || needs.changes.outputs.deny == 'true'",
            "    if: needs.changes.outputs.rust == 'true'",
            vec![Drift::Trigger {
                job: "checks",
                table: names(&["deny", "rust"]),
                workflow: names(&["rust"]),
            }],
        ),
        (
            "  docs:\n",
            "  docs:\n    if: needs.changes.outputs.web == 'true'\n",
            vec![Drift::UnknownOutput {
                job: "docs".to_owned(),
                output: "web".to_owned(),
            }],
        ),
        // An output built from two filters reads both.
        (
            "steps.filter.outputs.ui == 'true'",
            "(steps.filter.outputs.ui == 'true' || steps.filter.outputs.docs == 'true')",
            vec![Drift::Trigger {
                job: "ui",
                table: names(&["ui"]),
                workflow: names(&["docs", "ui"]),
            }],
        ),
    ];
    for (from, to, drifts) in cases {
        assert!(WORKFLOW.contains(from), "{from}");
        let workflow = Workflow::parse(&WORKFLOW.replace(from, to)).expect("the workflow parses");
        assert_eq!(CI.drifts(&workflow), drifts, "{from} -> {to}");
    }
}

#[test]
fn the_quantifier_is_read_from_the_paths_filter_step() {
    let every = WORKFLOW.replace(
        "          filters:",
        "          predicate-quantifier: every\n          filters:",
    );
    let workflow = Workflow::parse(&every).expect("the workflow parses");
    assert_eq!(workflow.quantifier, Quantifier::Every);

    let unknown = WORKFLOW.replace(
        "          filters:",
        "          predicate-quantifier: all\n          filters:",
    );
    assert!(matches!(
        Workflow::parse(&unknown),
        Err(Error::UnknownQuantifier { found }) if found == "all"
    ));
}

#[test]
fn the_paths_filter_step_must_read_the_filters_file() {
    let inline = WORKFLOW.replace(
        "filters: .github/filters.yml",
        "filters: |\n            rust: ['**']",
    );
    assert!(matches!(
        Workflow::parse(&inline),
        Err(Error::InlineFilters)
    ));
}
