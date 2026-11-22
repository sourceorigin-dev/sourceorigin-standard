//! `boxed_dyn_error_in_api`'s cases: a `dyn Error` in a public signature,
//! field or type alias of a library is flagged; borrowed ones, private
//! items, trait impls and other trait objects are not.
#![crate_type = "lib"]

use std::error::Error;
use std::error::Error as StdError;
use std::fmt;

pub type BoxError = Box<dyn Error + Send + Sync>;

pub fn load(path: &str) -> Result<String, Box<dyn Error>> {
    Ok(path.to_owned())
}

pub fn report(error: &dyn Error) -> String {
    error.to_string()
}

pub fn shared() -> std::sync::Arc<dyn Error + Send + Sync> {
    std::sync::Arc::new(fmt::Error)
}

pub struct Failure {
    pub cause: Box<dyn Error + Send + Sync>,
    hidden: Box<dyn Error>,
}

impl Failure {
    pub fn new(cause: BoxError) -> Self {
        Self {
            hidden: Box::new(fmt::Error),
            cause,
        }
    }

    fn private(&self) -> &dyn Error {
        &*self.hidden
    }
}

pub trait Source {
    fn source(&self) -> Option<&(dyn Error + 'static)>;
}

impl Source for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

fn private(path: &str) -> Result<String, Box<dyn Error>> {
    Ok(path.to_owned())
}

pub fn callback(f: Box<dyn Fn() -> u8>) -> u8 {
    f()
}

pub trait ErrorHandler {
    fn handle(&self);
}

pub fn handlers(handler: Box<dyn ErrorHandler>) {
    handler.handle();
}

pub fn renamed() -> Box<dyn StdError> {
    Box::new(fmt::Error)
}

type PrivateBox = Box<dyn Error + Send + Sync>;

pub fn through_private_alias() -> PrivateBox {
    Box::new(fmt::Error)
}

pub trait Domain: Error {}

pub fn subtrait(error: Box<dyn Domain>) -> String {
    error.to_string()
}

pub fn borrowed_returning(make: &dyn Fn() -> Box<dyn Error>) -> String {
    make().to_string()
}

pub struct Port(pub u16);

impl std::str::FromStr for Port {
    type Err = Box<dyn Error>;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ok(Self(text.parse()?))
    }
}

#[expect(boxed_dyn_error_in_api, reason = "the case an exception covers")]
pub fn expected() -> Box<dyn Error> {
    Box::new(fmt::Error)
}
