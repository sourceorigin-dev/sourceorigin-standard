//! The repository a check runs in, and the commands it runs there.

use std::path::Path;
use std::process::{Command, ExitStatus};
use std::string::FromUtf8Error;

/// A command a check ran that failed.
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// It didn't start, as when the program isn't installed.
    #[error("run `{command}`")]
    Start {
        /// The command line.
        command: String,
        /// Why it didn't start.
        #[source]
        source: std::io::Error,
    },
    /// It exited unsuccessfully.
    #[error("`{command}` failed ({status}): {stderr}")]
    Failed {
        /// The command line.
        command: String,
        /// How it exited.
        status: ExitStatus,
        /// What it printed on standard error.
        stderr: String,
    },
    /// It printed something other than UTF-8.
    #[error("`{command}` printed something other than UTF-8")]
    Utf8 {
        /// The command line.
        command: String,
        /// Where the output stopped being UTF-8.
        #[source]
        source: FromUtf8Error,
    },
}

/// The root of the repository a check runs in, which is also its Cargo
/// workspace's root.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Workspace<'a> {
    root: &'a Path,
}

impl<'a> Workspace<'a> {
    pub(crate) fn new(root: &'a Path) -> Self {
        Self { root }
    }

    /// `program`, run from the root without the package variables
    /// `cargo run` gave the xtask. Build scripts such as ring's track
    /// `CARGO_MANIFEST_DIR`, so passing the xtask's on to cargo, directly or
    /// through `just`, would make every nested build, and the next ordinary
    /// one, recompile them and their dependents.
    pub(crate) fn command(self, program: &str) -> Command {
        let mut command = Command::new(program);
        command.current_dir(self.root);
        for (key, _) in std::env::vars_os() {
            if key.to_str().is_some_and(|key| {
                key.starts_with("CARGO_PKG_")
                    || matches!(
                        key,
                        "CARGO_MANIFEST_DIR"
                            | "CARGO_MANIFEST_PATH"
                            | "CARGO_CRATE_NAME"
                            | "CARGO_BIN_NAME"
                            | "CARGO_PRIMARY_PACKAGE"
                    )
            }) {
                command.env_remove(key);
            }
        }
        command
    }

    /// Runs `program` with `args` from the root, and returns what it printed.
    pub(crate) fn output(self, program: &str, args: &[&str]) -> Result<String, CommandError> {
        let command = [program]
            .iter()
            .chain(args)
            .copied()
            .collect::<Vec<_>>()
            .join(" ");
        let output = self
            .command(program)
            .args(args)
            .output()
            .map_err(|source| CommandError::Start {
                command: command.clone(),
                source,
            })?;
        if !output.status.success() {
            return Err(CommandError::Failed {
                command,
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        String::from_utf8(output.stdout).map_err(|source| CommandError::Utf8 { command, source })
    }
}
