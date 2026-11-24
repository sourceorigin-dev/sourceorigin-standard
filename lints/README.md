# The standard's Dylint lints

The rules that need types or name resolution, which Clippy's settings and the
crate's syntax checks can't express, as one [Dylint](https://github.com/trailofbits/dylint)
library, `standard_lints`. Each lint's message says what to write instead,
and how to challenge the lint when it is wrong.

| Lint | Flags |
| --- | --- |
| [`wildcard_enum_local`](src/wildcard_enum_local.rs) | a `_` or catch-all binding arm in a `match` on an enum of the workspace |
| [`string_literal_logic`](src/string_literal_logic.rs) | comparing or matching a value against a string literal or constant outside parsing, display and tests |
| [`error_source_not_marked`](src/error_source_not_marked.rs) | an error field of a `thiserror` type that isn't `#[source]` or `#[from]` |
| [`error_logged_with_debug`](src/error_logged_with_debug.rs) | an error a `tracing` event formats through `Debug` (`?error`, `{error:?}`) |
| [`free_fn_with_owner`](src/free_fn_with_owner.rs) | a free function whose first parameter is a type of the crate, which should be its method |
| [`boxed_dyn_error_in_api`](src/boxed_dyn_error_in_api.rs) | an owned `dyn Error` (`Box<dyn Error>`) in a library's public API |

## In a repository

The workspace's `Cargo.toml` loads the library at a commit, so a new lint or
a changed one reaches a repository only through a pull request there that
moves `rev`. It also declares the `dylint_lib` cfg its exceptions use:

```toml
[workspace.metadata.dylint]
libraries = [{ git = "https://github.com/sourceorigin-dev/standard", rev = "<commit>", pattern = "lints" }]

[workspace.lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ["cfg(dylint_lib, values(any()))"] }
```

`just lint` runs them with warnings denied, as `cargo dylint` only warns and
exits 0 otherwise; it needs `cargo install cargo-dylint dylint-link`:

```sh
DYLINT_RUSTFLAGS='-D warnings' cargo dylint --all -- --all-targets
```

An exception names the lint and says why it is wrong there, and compiles
without Dylint too:

```rust
#[cfg_attr(dylint_lib = "standard_lints", expect(wildcard_enum_local, reason = "..."))]
```

A lint that is wrong more than once is changed here instead.

`dylint.toml`, beside the workspace's `Cargo.toml`, configures the lints:

```toml
[error_logged_with_debug]
# The type that displays an error with its source chain, which the help names.
chain_helper = "ErrorChain"
```

## Developing

`lints/` is a crate of its own, on the nightly `rust-toolchain.toml` pins
with `rustc-dev`: Dylint, `clippy_utils` and that nightly move together.
Each lint is a module of `src/`, registered in `src/lib.rs`; `report`,
`error` and `code` hold what several lints share. A lint's doc says what it
flags and what to write instead, and `Report` adds how to challenge it.

`just lints`, from the repository's root, checks formatting, runs Clippy,
the UI tests and the audit, then runs the lints on this repository with
warnings denied. It runs in CI only, as it needs the pinned nightly and
`cargo-dylint` and `dylint-link` at the version `Cargo.toml` pins.

Each lint's UI cases are an example, `ui/<lint>.rs`, with flagged and
allowed cases, and `ui/<lint>.stderr` the diagnostics they produce with the
library's other lints allowed; `tests/ui.rs` runs one test per lint. When a
test fails, compiletest saves the actual output, and its path is in the
failure.
