use std::collections::BTreeSet;

use super::super::filters::Quantifier;
use super::super::modules::Modules;
use super::super::workflow::Workflow;
use super::super::{Drift, Error};
use super::{CI, Filter};
use crate::check_changed::{Ci, Job, Recipe, Run, Trigger};

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
    steps:
      - run: just fmt-check
  rust:
    needs: changes
    if: needs.changes.outputs.rust == 'true'
    strategy:
      matrix:
        include:
          - run: |
              just lint
              just openapi-check
          - run: just doc-test
    steps:
      - run: ${{ matrix.run }}
  ui:
    needs: changes
    if: needs.changes.outputs.ui == 'true'
    steps:
      - run: just ui-check
      - run: just test-playwright
  docs:
    runs-on: ubuntu-latest
    steps:
      - run: nix develop --command just docs
  ci:
    if: always()
    needs: [changes, checks, rust, ui, docs]
";

fn names(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(ToString::to_string).collect()
}

#[test]
fn a_table_matching_the_workflow_has_no_drift() {
    let workflow = Workflow::parse(WORKFLOW, Modules::default()).expect("the workflow parses");
    assert_eq!(workflow.quantifier, Quantifier::Some);
    assert_eq!(CI.drifts(&workflow), []);
}

/// Each kind of drift, including a trigger read through an output built
/// from two filters, which reads both.
#[test]
fn drift_between_the_table_and_the_workflow_is_named() {
    let cases = [
        (
            "  ui:\n    needs: changes\n    if: needs.changes.outputs.ui == 'true'\n    steps:\n      \
             - run: just ui-check\n      - run: just test-playwright\n",
            "",
            vec![Drift::Missing { job: "ui" }],
        ),
        (
            "  docs:\n",
            "  links:\n    runs-on: ubuntu-latest\n  docs:\n",
            vec![Drift::Unlisted {
                job: "links".to_owned(),
            }],
        ),
        (
            "      - run: just ui-check\n",
            "      - run: just ui-check --fast && just ui-build\n",
            vec![Drift::Recipes {
                job: "ui",
                table: names(&["test-playwright", "ui-check"]),
                workflow: names(&["test-playwright", "ui-build", "ui-check"]),
            }],
        ),
        (
            "  ci:\n    if: always()\n    needs: [changes, checks, rust, ui, docs]\n",
            "",
            vec![Drift::StaleIgnore { job: "ci" }],
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
        let workflow = Workflow::parse(&WORKFLOW.replace(from, to), Modules::default())
            .expect("the workflow parses");
        assert_eq!(CI.drifts(&workflow), drifts, "{from} -> {to}");
    }
}

/// A table whose `web` job runs a module's recipe, `just web test`.
const MODULE_CI: Ci<Filter> = Ci {
    jobs: &[Job {
        id: "web",
        trigger: Trigger::EveryChange,
        recipes: &[Recipe {
            command: &["just", "web", "test"],
            run: Run::Workspace,
        }],
    }],
    ignored: &[],
};

/// A module's recipe compares by its whole path, so `just web` (the
/// module's default recipe) or `just web lint` isn't `just web test`, while
/// its arguments may still differ.
#[test]
fn a_module_recipe_compares_by_its_path() {
    let modules = || {
        serde_json::from_str::<Modules>(
            r#"{"modules": {"web": {"recipes": {"lint": {}, "test": {}}}}}"#,
        )
        .expect("the modules parse")
    };
    let drift = |workflow: &[&str]| {
        vec![Drift::Recipes {
            job: "web",
            table: names(&["web test"]),
            workflow: names(workflow),
        }]
    };
    for (run, drifts) in [
        ("just web test --fast", vec![]),
        ("just web::test", vec![]),
        ("just web", drift(&["web"])),
        ("just web && just web lint", drift(&["web", "web lint"])),
    ] {
        let yaml = format!(
            "jobs:\n  changes:\n    steps:\n      - id: filter\n        uses: \
             dorny/paths-filter@v4\n        with:\n          filters: .github/filters.yml\n  \
             web:\n    steps:\n      - run: {run}\n"
        );
        let workflow = Workflow::parse(&yaml, modules()).expect("the workflow parses");
        assert_eq!(MODULE_CI.drifts(&workflow), drifts, "{run}");
    }
}

#[test]
fn the_quantifier_is_read_from_the_paths_filter_step() {
    let every = WORKFLOW.replace(
        "          filters:",
        "          predicate-quantifier: every\n          filters:",
    );
    let workflow = Workflow::parse(&every, Modules::default()).expect("the workflow parses");
    assert_eq!(workflow.quantifier, Quantifier::Every);

    let unknown = WORKFLOW.replace(
        "          filters:",
        "          predicate-quantifier: all\n          filters:",
    );
    assert!(matches!(
        Workflow::parse(&unknown, Modules::default()),
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
        Workflow::parse(&inline, Modules::default()),
        Err(Error::InlineFilters)
    ));
}
