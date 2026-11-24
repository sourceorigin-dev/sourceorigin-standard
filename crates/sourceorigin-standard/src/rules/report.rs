//! `report`: what the monthly cleanup pass reads of a repository, as JSON.

use std::collections::BTreeMap;

use serde::Serialize;

use super::comments::Comment;
use super::level::Level;
use super::workspace_lints::LintLevel;
use super::{Repository, Rule, Violation};

/// What `cargo xtask standard report` prints: each rule's violations and
/// deviation, and the repository's exceptions, so the cleanup pass can find
/// the rules that cause the most friction across repositories.
#[derive(Debug, Serialize)]
pub struct Report {
    /// Each rule, by name.
    pub rules: BTreeMap<&'static str, RuleReport>,
    /// Each lint `#[expect]` attributes name, with how many name it.
    pub expects: BTreeMap<String, usize>,
    /// How many `// INVARIANT:` lines the repository holds.
    pub invariants: usize,
    /// The lints `[workspace.lints]` allows, such as
    /// `clippy::module_name_repetitions`.
    pub workspace_allows: Vec<String>,
}

/// One rule in a [`Report`].
#[derive(Debug, Serialize)]
pub struct RuleReport {
    /// Where the repository breaks it, deviation or not.
    pub violations: Vec<Violation>,
    /// The reason `standard.toml` gives for departing from it, if it does.
    pub deviation: Option<String>,
}

impl Report {
    /// The report as pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// When `serde_json` fails, which a report's maps of strings and
    /// numbers don't make it do.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

impl Repository {
    /// The report of every rule and exception.
    pub(super) fn report(&self) -> Report {
        let rules = Rule::ALL
            .iter()
            .map(|&rule| {
                let violations = rule
                    .check(self)
                    .findings
                    .into_iter()
                    .map(|finding| finding.of(rule))
                    .collect();
                let deviation = self
                    .config
                    .deviation(rule)
                    .map(|deviation| deviation.reason.clone());
                (
                    rule.name(),
                    RuleReport {
                        violations,
                        deviation,
                    },
                )
            })
            .collect();
        let mut expects = BTreeMap::new();
        for lint in self.sources.iter().flat_map(|source| &source.lints) {
            if lint.level == Level::Expect {
                for name in &lint.lints {
                    *expects.entry(name.clone()).or_default() += 1;
                }
            }
        }
        let invariants = self
            .sources
            .iter()
            .flat_map(|source| &source.comments)
            .map(Comment::invariants)
            .sum();
        let workspace_allows = self
            .lint_levels
            .iter()
            .filter(|lint| lint.level == Level::Allow)
            .map(LintLevel::lint)
            .collect();
        Report {
            rules,
            expects,
            invariants,
            workspace_allows,
        }
    }
}
