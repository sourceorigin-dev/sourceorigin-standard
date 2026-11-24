//! `wildcard_enum_local`'s cases: a catch-all arm on an enum this crate
//! defines is flagged; one on a foreign enum, or a match naming every
//! variant, is not.

use std::io::ErrorKind;

enum Kind {
    Unit,
    Tuple(u8),
    Struct { value: u8 },
}

fn wildcard(kind: &Kind) -> u8 {
    match kind {
        Kind::Unit => 0,
        _ => 1,
    }
}

fn binding(kind: Kind) -> u8 {
    match kind {
        Kind::Tuple(value) => value,
        other => wildcard(&other),
    }
}

fn bound_wildcard(kind: Kind) -> u8 {
    match kind {
        Kind::Unit => 0,
        other @ _ => wildcard(&other),
    }
}

fn partly_covered(kind: &Kind) -> u8 {
    match kind {
        Kind::Tuple(1) => 1,
        Kind::Struct { .. } => 2,
        _ => 0,
    }
}

fn every_variant(kind: &Kind) -> u8 {
    match kind {
        Kind::Unit | Kind::Tuple(_) => 0,
        Kind::Struct { value } => *value,
    }
}

fn guarded(kind: &Kind, flag: bool) -> u8 {
    match kind {
        Kind::Unit => 0,
        _ if flag => 1,
        Kind::Tuple(_) | Kind::Struct { .. } => 2,
    }
}

fn only_catch_all(kind: &Kind) -> u8 {
    match kind {
        _ => 0,
    }
}

fn foreign(kind: ErrorKind) -> u8 {
    match kind {
        ErrorKind::NotFound => 0,
        _ => 1,
    }
}

fn option(kind: Option<Kind>) -> u8 {
    match kind {
        Some(Kind::Unit) => 0,
        _ => 1,
    }
}

fn from_macro(kind: &Kind) -> bool {
    matches!(kind, Kind::Unit)
}

#[expect(wildcard_enum_local, reason = "the case an exception covers")]
fn expected(kind: &Kind) -> u8 {
    match kind {
        Kind::Unit => 0,
        _ => 1,
    }
}

fn main() {}
