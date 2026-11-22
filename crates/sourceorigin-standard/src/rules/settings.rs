//! Flat TOML files of `key = value` settings, such as `rustfmt.toml`, and
//! the values the rules require of them.

use std::collections::BTreeMap;

use toml::Spanned;

use super::Finding;
use super::fix::{Checked, Fix};
use super::toml_file::TomlFile;

/// A flat TOML file's settings.
pub(super) type Settings = BTreeMap<Spanned<String>, toml::Value>;

/// A value a setting must have.
#[derive(Clone, Copy)]
pub(super) enum Value {
    Str(&'static str),
    Bool(bool),
}

impl Value {
    fn is(self, value: &toml::Value) -> bool {
        match self {
            Self::Str(wanted) => value.as_str() == Some(wanted),
            Self::Bool(wanted) => value.as_bool() == Some(wanted),
        }
    }

    /// The value, for a TOML document.
    fn toml(self) -> toml_edit::Value {
        match self {
            Self::Str(wanted) => wanted.into(),
            Self::Bool(wanted) => wanted.into(),
        }
    }

    /// The value as TOML writes it.
    fn literal(self) -> String {
        match self {
            Self::Str(wanted) => format!("\"{wanted}\""),
            Self::Bool(wanted) => wanted.to_string(),
        }
    }
}

impl TomlFile<Settings> {
    /// The setting `key`, with its line.
    pub(super) fn setting(&self, key: &str) -> Option<(usize, &toml::Value)> {
        self.value
            .iter()
            .find(|(name, _)| name.get_ref() == key)
            .map(|(name, value)| (self.line(name), value))
    }

    /// Each of `required` the file doesn't set to its value, fixed by
    /// setting it.
    pub(super) fn settings_are(&self, required: &[(&'static str, Value)]) -> Checked {
        let mut checked = Checked::default();
        for &(key, wanted) in required {
            let setting = format!("{key} = {}", wanted.literal());
            let (line, message) = match self.setting(key) {
                Some((_, value)) if wanted.is(value) => continue,
                Some((line, _)) => (line, format!("`{key}` has another value: set `{setting}`")),
                None => (1, format!("`{key}` is unset: add `{setting}`")),
            };
            checked.findings.push(Finding {
                path: self.path.clone(),
                line,
                message,
            });
            checked.fixes.push(Fix::new(
                self.path.clone(),
                format!("set `{setting}`"),
                move |document| {
                    let new = wanted.toml();
                    if document.contains_key(key) {
                        Fix::set(&mut document[key], new);
                    } else {
                        document[key] = toml_edit::Item::Value(new);
                    }
                },
            ));
        }
        checked
    }
}
