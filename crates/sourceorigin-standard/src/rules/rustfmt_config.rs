//! `rustfmt-config`: rustfmt formats every repository alike.

use super::Repository;
use super::fix::Checked;
use super::settings::Value;

/// rustfmt's configuration, from the repository's root.
pub(super) const PATH: &str = "rustfmt.toml";

/// The settings it requires.
const REQUIRED: [(&str, Value); 3] = [
    ("edition", Value::Str("2024")),
    ("newline_style", Value::Str("Unix")),
    ("use_field_init_shorthand", Value::Bool(true)),
];

impl Repository {
    /// Each required setting `rustfmt.toml` lacks or sets otherwise.
    pub(super) fn rustfmt_config(&self) -> Checked {
        self.rustfmt.settings_are(&REQUIRED)
    }
}
