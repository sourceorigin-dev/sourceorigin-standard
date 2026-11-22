//! `string_literal_logic`'s cases: logic on string literals is flagged;
//! parsing, display, serde and test code is not.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserializer, Visitor};

enum Mode {
    Fast,
    Slow,
}

fn equals(mode: &str) -> bool {
    mode == "fast"
}

fn not_equals(mode: String) -> bool {
    "slow" != mode
}

fn matched(mode: &str) -> u8 {
    match mode {
        "fast" => 0,
        "slow" | "slower" => 1,
        _ => 2,
    }
}

fn prefixed(path: &str) -> bool {
    path.starts_with("/api") || path.to_owned().ends_with(".json")
}

const FAST: &str = "fast";
static SLOW: &str = "slow";

fn constants(mode: &str) -> bool {
    mode == FAST || SLOW != mode
}

fn constant_pattern(mode: &str) -> u8 {
    match mode {
        FAST => 0,
        _ => 1,
    }
}

fn optional(mode: Option<&str>) -> bool {
    mode == Some("fast")
}

fn split(header: &str) -> Option<&str> {
    header
        .strip_prefix("Bearer ")
        .or_else(|| header.split_once(": ").map(|(_, value)| value))
}

fn empty(mode: &str) -> bool {
    mode == ""
}

fn chars(mode: &str) -> bool {
    mode.starts_with('/')
}

fn from_macro(mode: &str) -> bool {
    matches!(mode, "fast")
}

macro_rules! names {
    ($($variant:ident => $name:literal),+) => {
        fn mode_named(name: &str) -> Option<Mode> {
            match name {
                $($name => Some(Mode::$variant),)+
                _ => None,
            }
        }
    };
}

names!(Fast => "fast", Slow => "slow");

impl FromStr for Mode {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "fast" => Ok(Self::Fast),
            "slow" => Ok(Self::Slow),
            other => Err(format!("unknown mode {other}")),
        }
    }
}

impl From<&str> for Mode {
    fn from(text: &str) -> Self {
        if text == "fast" {
            Self::Fast
        } else {
            Self::Slow
        }
    }
}

impl Mode {
    fn named(name: &str) -> Option<Self> {
        if name == "fast" {
            Some(Self::Fast)
        } else {
            None
        }
    }

    fn is_fast_name(name: &str) -> bool {
        name == "fast"
    }
}

impl TryFrom<&String> for Mode {
    type Error = String;

    fn try_from(text: &String) -> Result<Self, Self::Error> {
        if text.eq_ignore_ascii_case("fast") {
            Ok(Self::Fast)
        } else {
            text.parse()
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Fast => "fast",
            Self::Slow => "slow",
        };
        if name == "fast" {
            f.write_str("FAST")
        } else {
            f.write_str(name)
        }
    }
}

struct ModeVisitor;

impl Visitor<'_> for ModeVisitor {
    type Value = Mode;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a mode")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Mode, E> {
        if text == "fast" {
            Ok(Mode::Fast)
        } else {
            Ok(Mode::Slow)
        }
    }
}

fn deserialize_mode<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Mode, D::Error> {
    let fast = deserializer.deserialize_str(ModeVisitor)?;
    Ok(fast)
}

#[cfg(test)]
mod tests {
    fn in_test(mode: &str) -> bool {
        mode == "fast"
    }
}

#[expect(string_literal_logic, reason = "the case an exception covers")]
fn expected(mode: &str) -> bool {
    mode == "fast"
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--fast") {
        let _ = Mode::Fast;
    }
}
