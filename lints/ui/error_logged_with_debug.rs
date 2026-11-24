//! `error_logged_with_debug`'s cases: an error a `tracing` field formats with
//! `?` is flagged; `%`, and `?` on other values, are not.

use std::error::Error;
use std::io;
use std::sync::Arc;

#[derive(Debug)]
struct Request {
    path: String,
}

fn log(error: &io::Error, boxed: Box<dyn Error + Send + Sync>, shared: Arc<io::Error>) {
    tracing::error!(?error, "read failed");
    tracing::warn!(error = ?boxed, "plugin failed");
    tracing::info!(cause = ?shared, "retrying");
    let source: &dyn Error = error;
    tracing::debug!(?source, "source");

    tracing::error!(%error, "read failed");
    let request = Request {
        path: String::from("/"),
    };
    tracing::info!(?request, path = %request.path, "request");
    let missing: Option<io::Error> = None;
    tracing::info!(?missing, "maybe");

    tracing::error!("read failed: {error:?}");
    tracing::warn!("plugin failed: {:?}", boxed);
    tracing::info!("maybe: {missing:?}");
    let result: Result<(), io::Error> = Ok(());
    tracing::info!("result: {result:?}");

    tracing::error!("read failed: {error}");
    tracing::info!("request: {request:?}");
    println!("not tracing: {error:?}");
}

fn expected(error: &io::Error) {
    #[expect(error_logged_with_debug, reason = "the case an exception covers")]
    {
        tracing::error!(?error, "read failed");
    }
}

fn main() {}
