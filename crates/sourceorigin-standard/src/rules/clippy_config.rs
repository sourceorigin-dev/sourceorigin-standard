//! `clippy-config`: tests may expect, panic and print, but never unwrap.

use super::fix::{Checked, Fix};
use super::settings::Value;
use super::{Finding, Repository};

/// Clippy's configuration, from the repository's root.
pub(super) const PATH: &str = "clippy.toml";

/// The settings it requires.
const REQUIRED: [(&str, Value); 3] = [
    ("allow-expect-in-tests", Value::Bool(true)),
    ("allow-panic-in-tests", Value::Bool(true)),
    ("allow-dbg-in-tests", Value::Bool(true)),
];

/// The setting it may not turn on.
const UNWRAP: &str = "allow-unwrap-in-tests";

impl Repository {
    /// Each required setting `clippy.toml` lacks or sets otherwise, and
    /// `allow-unwrap-in-tests = true`, which the fix removes.
    pub(super) fn clippy_config(&self) -> Checked {
        let mut checked = self.clippy.settings_are(&REQUIRED);
        if let Some((line, value)) = self.clippy.setting(UNWRAP)
            && value.as_bool() != Some(false)
        {
            checked.findings.push(Finding {
                path: PATH.into(),
                line,
                message: format!(
                    "`{UNWRAP}` lets tests unwrap: remove it, and `expect` what should hold"
                ),
            });
            checked
                .fixes
                .push(Fix::new(PATH, format!("remove `{UNWRAP}`"), |document| {
                    document.remove(UNWRAP);
                }));
        }
        checked
    }
}
