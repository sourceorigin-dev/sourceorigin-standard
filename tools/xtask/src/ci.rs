//! The jobs of .github/workflows/ci.yml, as `just check-changed` runs them.
//! The `ci_matches_the_workflow` test fails until this table and the
//! workflow change together.

use sourceorigin_standard::check_changed::{self, Ci, Job, Recipe, Run, Trigger};

/// A filter of .github/filters.yml.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Filter {
    Workspace,
    Rust,
    Deny,
    PrChecks,
}

impl check_changed::Filter for Filter {
    const ALL: &'static [Self] = &[Self::Workspace, Self::Rust, Self::Deny, Self::PrChecks];

    fn name(self) -> &'static str {
        match self {
            Self::Workspace => "workspace",
            Self::Rust => "rust",
            Self::Deny => "deny",
            Self::PrChecks => "pr-checks",
        }
    }
}

/// A recipe of the justfile, on the whole workspace: with one published
/// crate, which this xtask depends on, every Rust change reaches both.
const fn just(recipe: &'static [&'static str]) -> Recipe {
    Recipe {
        command: recipe,
        run: Run::Workspace,
    }
}

/// CI's jobs. Every check is quick, so none is left to CI.
pub const CI: Ci<Filter> = Ci {
    jobs: &[
        Job {
            id: "checks",
            trigger: Trigger::AnyOf(&[Filter::Rust, Filter::Deny]),
            recipes: &[
                just(&["just", "fmt-check"]),
                just(&["just", "audit"]),
                just(&["just", "unused"]),
            ],
        },
        Job {
            id: "standard",
            trigger: Trigger::EveryChange,
            recipes: &[just(&["just", "standard"])],
        },
        Job {
            id: "rust",
            trigger: Trigger::AnyOf(&[Filter::Rust]),
            recipes: &[
                just(&["just", "lint"]),
                just(&["just", "doc"]),
                just(&["just", "test"]),
                just(&["just", "doc-test"]),
                just(&["just", "package"]),
            ],
        },
        Job {
            id: "pr-check-scripts",
            trigger: Trigger::AnyOf(&[Filter::PrChecks]),
            recipes: &[just(&["just", "test-pr-checks"])],
        },
    ],
    ignored: &["ci"],
};

#[cfg(test)]
mod tests {
    #[test]
    fn ci_matches_the_workflow() {
        super::CI
            .check_workflow(crate::root())
            .expect("the CI table matches ci.yml and filters.yml");
    }
}
