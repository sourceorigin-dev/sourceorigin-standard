#![doc = include_str!("../README.md")]

pub mod check_changed;
mod config;
mod metadata;
pub mod rules;
#[cfg(test)]
mod testing;
mod workspace;

pub use config::ConfigError;
pub use metadata::MetadataError;
pub use workspace::CommandError;
