# Rust conventions

sourceorigin-xtask follows the shared
[engineering standard](https://gist.github.com/PiquelChips/04330e52c03efbe9ce4c889c88673a27),
also used by piqueld, sourceorigin and DirkEngine. This page applies it to
the crate. Keep implementations simple, follow neighbouring modules, and
share repeated logic instead of copying it.

## What belongs here

The crate holds what every repository's xtask would otherwise copy, and
nothing a single repository needs: its jobs, filters and recipes stay in its
own xtask, as typed values the crate reads. Add a feature here only once a
second repository needs it, and keep the API as small as that use allows.

## Errors

The crate is a library: every fallible function returns its feature's
`thiserror` error enum, keeping each source with `#[source]` or `#[from]`.
`anyhow` belongs to a repository's xtask, at its edge.

Production code never panics: no `unwrap`, `expect`, `panic!`, `todo!` or
`unimplemented!`. Tests may panic, through an `expect` that says what should
have held; `unwrap` is never allowed (`clippy.toml`).

## Lint exceptions

Never `#[allow]`. An exception is `#[expect(lint, reason = "...")]` on the
smallest item it applies to, and the reason says why the lint is wrong
there.

## Tests

Unit tests live in a sibling `tests.rs`, or a `tests/` directory once they
outgrow one read. A test proves one behaviour and is named for it. Glob
semantics are tested against what dorny/paths-filter does, case by case, with
the version each case was checked against.

Tests wait for events, not time: no `thread::sleep` or `tokio::time::sleep`
to let something happen. Await the channel or state change that marks the
milestone, and move time with `#[tokio::test(start_paused = true)]`. A wait
on a real subprocess says why on the line above, with
`// wall-clock: <why>`. `cargo xtask test-waits`, which `just lint` runs,
enforces this.

## Releasing

The owner publishes to crates.io. `just package` packages the crate and
builds it from its package, as crates.io would, without uploading anything;
CI runs it on every change.

## Deviations

- No repository skills (`.agents/skills`): the crate has no application to
  run and no documentation site, so there is no workflow a skill would hold.
