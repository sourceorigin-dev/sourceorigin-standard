//! `error_source_not_marked`'s cases: an error field of a `thiserror` type
//! that isn't its source is flagged; a marked one, or a type without
//! `#[error]`, is not.

use std::error::Error;
use std::io;
use std::num::ParseIntError;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
#[error("read {path}")]
struct ReadError {
    path: String,
    io: io::Error,
}

#[derive(Debug, thiserror::Error)]
enum ConfigError {
    #[error("parse the port: {0}")]
    Port(ParseIntError),
    #[error("load the plugin")]
    Plugin(Box<dyn Error + Send + Sync>),
    #[error("shared")]
    Shared { cause: Option<Arc<io::Error>> },
    #[error("read the file")]
    Marked(#[source] io::Error),
    #[error("convert")]
    Converted(#[from] ReadError),
    #[error("named source")]
    Named { source: io::Error, path: String },
    #[error(transparent)]
    Transparent(ParseIntError),
    #[error("second error")]
    Second {
        #[source]
        first: io::Error,
        second: ParseIntError,
    },
    #[error("no error field: {0}")]
    Message(String),
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
struct Wrapper(io::Error);

#[derive(Debug, thiserror::Error)]
#[error("storage failed")]
enum Inherited {
    Read(io::Error),
    #[error("parse: {0}")]
    Parse(ParseIntError),
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
enum Forwarded {
    Io(io::Error),
    Parse(ParseIntError),
}

#[derive(Debug)]
struct NotThiserror {
    io: io::Error,
}

#[derive(Debug, thiserror::Error)]
#[error("expected")]
struct Expected {
    #[expect(error_source_not_marked, reason = "the case an exception covers")]
    io: io::Error,
}

fn main() {}
