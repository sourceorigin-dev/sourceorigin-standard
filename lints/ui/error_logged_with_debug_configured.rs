//! `error_logged_with_debug` with `chain_helper` set: the help names it.

fn log(error: &std::io::Error) {
    tracing::error!(?error, "read failed");
}

fn main() {}
