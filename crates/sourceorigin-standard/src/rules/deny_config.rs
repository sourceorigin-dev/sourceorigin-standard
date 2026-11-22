//! `deny-config`: cargo-deny checks where crates come from, what they
//! require, and their licenses, and says why it ignores an advisory.

use serde::Deserialize;
use toml::Spanned;

use super::fix::{Checked, Fix};
use super::toml_file::TomlFile;
use super::{Finding, Repository};

/// cargo-deny's configuration, from the repository's root.
pub(super) const PATH: &str = "deny.toml";

/// `deny.toml`, reduced to what the rule reads.
#[derive(Deserialize)]
pub(super) struct DenyConfig {
    advisories: Option<Advisories>,
    licenses: Option<Spanned<Licenses>>,
    bans: Option<Spanned<Bans>>,
    sources: Option<Spanned<Sources>>,
}

#[derive(Deserialize)]
struct Advisories {
    #[serde(default)]
    ignore: Vec<Spanned<toml::Value>>,
}

#[derive(Deserialize)]
struct Licenses {
    #[serde(default)]
    allow: Vec<toml::Value>,
}

#[derive(Deserialize)]
struct Bans {
    wildcards: Option<Spanned<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Sources {
    unknown_registry: Option<Spanned<String>>,
    unknown_git: Option<Spanned<String>>,
}

/// A setting that must be `"deny"`.
struct Denied<'a> {
    table: &'static str,
    /// Where its table starts, if there is one.
    table_at: Option<usize>,
    key: &'static str,
    value: Option<&'a Spanned<String>>,
}

impl DenyConfig {
    /// The settings that must be `"deny"`.
    fn denied(&self) -> [Denied<'_>; 3] {
        let sources = self.sources.as_ref();
        let bans = self.bans.as_ref();
        let sources_at = sources.map(|sources| sources.span().start);
        [
            Denied {
                table: "sources",
                table_at: sources_at,
                key: "unknown-registry",
                value: sources.and_then(|sources| sources.get_ref().unknown_registry.as_ref()),
            },
            Denied {
                table: "sources",
                table_at: sources_at,
                key: "unknown-git",
                value: sources.and_then(|sources| sources.get_ref().unknown_git.as_ref()),
            },
            Denied {
                table: "bans",
                table_at: bans.map(|bans| bans.span().start),
                key: "wildcards",
                value: bans.and_then(|bans| bans.get_ref().wildcards.as_ref()),
            },
        ]
    }
}

impl TomlFile<DenyConfig> {
    /// Each setting that must be `"deny"` and isn't, fixed by setting it,
    /// an empty license allowlist, and each ignored advisory without a
    /// `reason`.
    fn checked(&self) -> Checked {
        let mut checked = Checked::default();
        let finding = |line, message: String| Finding {
            path: PATH.into(),
            line,
            message,
        };
        for Denied {
            table,
            table_at,
            key,
            value,
        } in self.value.denied()
        {
            let setting = format!("{key} = \"deny\"");
            let line = match value {
                Some(value) if value.get_ref() == "deny" => continue,
                Some(value) => self.line(value),
                None => table_at.map_or(1, |offset| self.line_at(offset)),
            };
            checked.findings.push(finding(
                line,
                format!("`[{table}]` doesn't deny `{key}`: set `{setting}`"),
            ));
            checked.fixes.push(Fix::toml(
                PATH,
                format!("set `{setting}`"),
                move |document| {
                    if document.get(table).is_none() {
                        document[table] = toml_edit::table();
                    }
                    let item = &mut document[table][key];
                    if item.is_none() {
                        *item = toml_edit::value("deny");
                    } else {
                        Fix::set(item, "deny");
                    }
                },
            ));
        }
        let licenses = self.value.licenses.as_ref();
        if licenses.is_none_or(|licenses| licenses.get_ref().allow.is_empty()) {
            checked.findings.push(finding(
                licenses.map_or(1, |licenses| self.line(licenses)),
                "`[licenses]` allows no license: list the licenses the repository accepts in \
                 `allow`"
                    .to_owned(),
            ));
        }
        let ignored = self
            .value
            .advisories
            .iter()
            .flat_map(|advisories| &advisories.ignore);
        for advisory in ignored {
            let reason = advisory
                .get_ref()
                .get("reason")
                .and_then(toml::Value::as_str);
            if reason.is_none_or(|reason| reason.trim().is_empty()) {
                let id = advisory
                    .get_ref()
                    .get("id")
                    .or(Some(advisory.get_ref()))
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default();
                checked.findings.push(finding(
                    self.line(advisory),
                    format!(
                        "the ignored advisory `{id}` gives no reason: write it as \
                         `{{ id = \"{id}\", reason = \"<why it doesn't apply, and what ends the \
                         ignore>\" }}`"
                    ),
                ));
            }
        }
        checked
    }
}

impl Repository {
    /// What `deny.toml` lacks of the standard, or a missing `deny.toml`.
    pub(super) fn deny_config(&self) -> Checked {
        match &self.deny {
            Some(deny) => deny.checked(),
            None if self.unreadable.contains(PATH) => Checked::default(),
            None => vec![Finding {
                path: PATH.into(),
                line: 1,
                message: format!("no {PATH}: configure cargo-deny there, which `just audit` runs"),
            }]
            .into(),
        }
    }
}
