//! Fixtures shared by the tests of each flow: a repository's filters and
//! its CI table.

mod changed_paths;
mod filters;
mod plan;
mod scope;
mod workflow;

use std::collections::BTreeSet;

use super::{Ci, Job, Recipe, Run, Trigger};

/// A repository's filters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Filter {
    Workspace,
    Crates,
    Rust,
    Deny,
    Ui,
    Docs,
}

impl super::Filter for Filter {
    const ALL: &'static [Self] = &[
        Self::Workspace,
        Self::Crates,
        Self::Rust,
        Self::Deny,
        Self::Ui,
        Self::Docs,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Crates => "crates",
            Self::Rust => "rust",
            Self::Deny => "deny",
            Self::Ui => "ui",
            Self::Docs => "docs",
        }
    }
}

/// Filters naming every [`Filter`], `rust` reaching `workspace` through an
/// alias of an alias.
const FILTERS: &str = "
workspace: &workspace
  - Cargo.lock
crates: &crates
  - *workspace
  - crates/core/**
rust:
  - *crates
  - apps/**
deny: ['**/Cargo.toml']
ui: [apps/ui/**]
docs: [docs/**]
";

const FMT_CHECK: Recipe = Recipe {
    command: &["just", "fmt-check"],
    run: Run::Workspace,
};
const LINT: Recipe = Recipe {
    command: &["just", "lint"],
    run: Run::Crates {
        libraries_only: false,
    },
};
const OPENAPI_CHECK: Recipe = Recipe {
    command: &["just", "openapi-check"],
    run: Run::Only(&["daemon", "cli"]),
};
const DOC_TEST: Recipe = Recipe {
    command: &["just", "doc-test"],
    run: Run::Crates {
        libraries_only: true,
    },
};
const UI_CHECK: Recipe = Recipe {
    command: &["just", "ui-check"],
    run: Run::Workspace,
};
const PLAYWRIGHT: Recipe = Recipe {
    command: &["just", "test-playwright"],
    run: Run::InCi("slow, so drafts skip it too"),
};
const DOCS: Recipe = Recipe {
    command: &["just", "docs"],
    run: Run::Workspace,
};

/// A repository's CI, as its xtask describes it.
const CI: Ci<Filter> = Ci {
    jobs: &[
        Job {
            id: "checks",
            trigger: Trigger::AnyOf(&[Filter::Rust, Filter::Deny]),
            recipes: &[FMT_CHECK],
        },
        Job {
            id: "rust",
            trigger: Trigger::AnyOf(&[Filter::Rust]),
            recipes: &[LINT, OPENAPI_CHECK, DOC_TEST],
        },
        Job {
            id: "ui",
            trigger: Trigger::AnyOf(&[Filter::Ui]),
            recipes: &[UI_CHECK, PLAYWRIGHT],
        },
        Job {
            id: "docs",
            trigger: Trigger::EveryChange,
            recipes: &[DOCS],
        },
    ],
    ignored: &["ci"],
};

fn paths(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(ToString::to_string).collect()
}
