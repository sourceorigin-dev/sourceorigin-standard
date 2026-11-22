//! `toolchain`: every repository builds with stable Rust, formatted and
//! linted.

use serde::Deserialize;
use toml::Spanned;

use super::fix::{Checked, Fix};
use super::{Finding, Repository};

/// The toolchain file, from the repository's root.
pub(super) const PATH: &str = "rust-toolchain.toml";

/// The channel it pins.
const CHANNEL: &str = "stable";

/// The components it installs, among others.
const COMPONENTS: [&str; 2] = ["rustfmt", "clippy"];

/// `rust-toolchain.toml`.
#[derive(Deserialize)]
pub(super) struct ToolchainFile {
    toolchain: Option<Spanned<Toolchain>>,
}

/// Its `[toolchain]`.
#[derive(Deserialize)]
struct Toolchain {
    channel: Option<Spanned<String>>,
    components: Option<Spanned<Vec<String>>>,
}

impl Repository {
    /// A missing toolchain file, which the fix writes, a channel other than
    /// stable, and each required component it doesn't install.
    pub(super) fn toolchain(&self) -> Checked {
        let Some(file) = &self.toolchain else {
            return Checked {
                findings: vec![Finding {
                    path: PATH.into(),
                    line: 1,
                    message: format!("no {PATH}: pin the `{CHANNEL}` channel there"),
                }],
                fixes: vec![Fix::new(
                    PATH,
                    format!("pin the `{CHANNEL}` toolchain"),
                    |document| {
                        document["toolchain"]["channel"] = toml_edit::value(CHANNEL);
                        document["toolchain"]["components"] =
                            toml_edit::value(COMPONENTS.into_iter().collect::<toml_edit::Array>());
                    },
                )],
            };
        };
        let finding = |line, message| Finding {
            path: PATH.into(),
            line,
            message,
        };
        let Some(toolchain) = &file.value.toolchain else {
            return vec![finding(
                1,
                format!("no `[toolchain]`: add one with `channel = \"{CHANNEL}\"`"),
            )]
            .into();
        };
        let mut findings = Vec::new();
        let channel = &toolchain.get_ref().channel;
        match channel {
            Some(channel) if channel.get_ref() == CHANNEL => {}
            _ => findings.push(finding(
                channel
                    .as_ref()
                    .map_or_else(|| file.line(toolchain), |channel| file.line(channel)),
                format!(
                    "the toolchain isn't the `{CHANNEL}` channel: set `channel = \"{CHANNEL}\"`, \
                     which rustup keeps current"
                ),
            )),
        }
        let components = toolchain.get_ref().components.as_ref();
        let line = components.map_or_else(|| file.line(toolchain), |list| file.line(list));
        findings.extend(
            COMPONENTS
                .iter()
                .filter(|&&component| {
                    !components.is_some_and(|list| list.get_ref().iter().any(|c| c == component))
                })
                .map(|component| {
                    finding(
                        line,
                        format!("the toolchain lacks the `{component}` component: add it to `components`"),
                    )
                }),
        );
        findings.into()
    }
}
