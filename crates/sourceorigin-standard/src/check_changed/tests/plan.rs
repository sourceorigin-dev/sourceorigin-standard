use std::collections::BTreeSet;

use super::super::change::{Crate, Outcome, Scope, Step};
use super::{CI, DOC_TEST, FMT_CHECK, LINT, OPENAPI_CHECK, PLAYWRIGHT, Recipe, UI_CHECK};

fn crates(names: &[(&str, bool)]) -> Scope {
    Scope::Crates(
        names
            .iter()
            .map(|&(name, library)| Crate {
                name: name.to_owned(),
                library,
            })
            .collect::<BTreeSet<_>>(),
    )
}

/// The command and outcome of `recipe` in the job `id`, for a change
/// reaching `scope`, or starting no job.
fn plan(id: &str, recipe: &Recipe, scope: Option<&Scope>) -> (String, Outcome) {
    let job = CI
        .jobs
        .iter()
        .find(|job| job.id == id)
        .expect("the job is in the table");
    let step = Step::plan(job, recipe, scope);
    (step.command.join(" "), step.outcome)
}

#[test]
fn checks_narrow_to_the_crates_the_change_reaches() {
    let scope = crates(&[("cli", true), ("xtask", false)]);
    let cases = [
        ("checks", FMT_CHECK, "just fmt-check", Outcome::ToRun),
        (
            "rust",
            LINT,
            "just lint --package=cli --package=xtask",
            Outcome::ToRun,
        ),
        (
            "rust",
            DOC_TEST,
            "just doc-test --package=cli",
            Outcome::ToRun,
        ),
        ("rust", OPENAPI_CHECK, "just openapi-check", Outcome::ToRun),
        (
            "ui",
            PLAYWRIGHT,
            "just test-playwright",
            Outcome::LeftToCi("slow, so drafts skip it too"),
        ),
    ];
    for (job, recipe, command, outcome) in cases {
        assert_eq!(
            plan(job, &recipe, Some(&scope)),
            (command.to_owned(), outcome),
            "{command}"
        );
    }
}

#[test]
fn checks_the_change_doesnt_need_are_skipped() {
    let xtask = crates(&[("xtask", false)]);
    assert_eq!(
        plan("rust", &DOC_TEST, Some(&xtask)).1,
        Outcome::Skipped("no crate the change reaches has a library".to_owned())
    );
    assert_eq!(
        plan("rust", &OPENAPI_CHECK, Some(&xtask)).1,
        Outcome::Skipped("the change reaches none of daemon, cli".to_owned())
    );
    assert_eq!(
        plan("ui", &UI_CHECK, None).1,
        Outcome::Skipped("no change matches `ui`".to_owned())
    );
}

#[test]
fn checks_run_on_the_workspace_when_a_change_reaches_it() {
    let scope = Scope::Workspace {
        path: "Cargo.lock".to_owned(),
    };
    assert_eq!(
        plan("rust", &LINT, Some(&scope)),
        ("just lint".to_owned(), Outcome::ToRun)
    );
}
