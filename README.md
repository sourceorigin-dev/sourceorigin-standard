# sourceorigin-standard

The engineering standard of piqueld, sourceorigin and DirkEngine, as code: each rule is enforced by a check rather than stated in prose. Each repository's `cargo xtask` depends on the [`sourceorigin-standard`](crates/sourceorigin-standard) crate and runs its checks, and the crate's README, which lists every rule, is the written standard. The rules that need types are [Dylint lints](lints) the repositories load from this repository.

Contributors start with [AGENTS.md](AGENTS.md).

Licensed under [Apache-2.0](LICENSE).
