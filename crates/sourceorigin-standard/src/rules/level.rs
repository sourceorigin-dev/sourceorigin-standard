//! A lint's level, as attributes and manifests set it.

use std::fmt;

use serde::Deserialize;

/// How an attribute or a manifest sets a lint's level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Level {
    Allow,
    Expect,
    Warn,
    Deny,
    Forbid,
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Allow => "allow",
            Self::Expect => "expect",
            Self::Warn => "warn",
            Self::Deny => "deny",
            Self::Forbid => "forbid",
        })
    }
}
