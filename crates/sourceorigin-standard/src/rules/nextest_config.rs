//! `nextest-config`: a hung test fails, and a flaky one isn't retried.

use std::collections::BTreeMap;

use serde::Deserialize;
use toml::Spanned;

use super::toml_file::TomlFile;
use super::{Finding, Repository};

/// nextest's configuration, from the repository's root.
pub(super) const PATH: &str = ".config/nextest.toml";

/// `.config/nextest.toml`, reduced to its profiles.
#[derive(Default, Deserialize)]
pub(super) struct NextestConfig {
    #[serde(default)]
    profile: BTreeMap<String, Profile>,
}

/// A profile, or one of its overrides.
#[derive(Deserialize)]
struct Profile {
    #[serde(rename = "slow-timeout")]
    slow_timeout: Option<Spanned<toml::Value>>,
    retries: Option<Spanned<toml::Value>>,
    #[serde(default)]
    overrides: Vec<Profile>,
}

impl Profile {
    /// It and its overrides.
    fn all(&self) -> impl Iterator<Item = &Self> {
        std::iter::once(self).chain(&self.overrides)
    }
}

impl TomlFile<NextestConfig> {
    /// A default profile without a `slow-timeout`, each `slow-timeout`
    /// without `terminate-after`, and each `retries`.
    fn findings(&self) -> Vec<Finding> {
        let finding = |line, message: String| Finding {
            path: PATH.into(),
            line,
            message,
        };
        let mut findings = Vec::new();
        let default = self.value.profile.get("default");
        if default.is_none_or(|profile| profile.slow_timeout.is_none()) {
            findings.push(finding(
                1,
                "`[profile.default]` sets no `slow-timeout`: add \
                 `slow-timeout = { period = \"60s\", terminate-after = 5 }`"
                    .to_owned(),
            ));
        }
        for profile in self.value.profile.values().flat_map(Profile::all) {
            if let Some(timeout) = &profile.slow_timeout
                && timeout.get_ref().get("terminate-after").is_none()
            {
                findings.push(finding(
                    self.line(timeout),
                    "a `slow-timeout` without `terminate-after` lets a hung test run forever: \
                     add `terminate-after`"
                        .to_owned(),
                ));
            }
            if let Some(retries) = &profile.retries {
                findings.push(finding(
                    self.line(retries),
                    "`retries` hides flaky tests: remove it, and fix the test".to_owned(),
                ));
            }
        }
        findings
    }
}

impl Repository {
    /// What `.config/nextest.toml` lacks, or allows, of the standard.
    pub(super) fn nextest_config(&self) -> Vec<Finding> {
        self.nextest
            .as_ref()
            .map_or_else(Vec::new, TomlFile::findings)
    }
}
