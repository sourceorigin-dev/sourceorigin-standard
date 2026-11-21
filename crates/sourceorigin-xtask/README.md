# sourceorigin-xtask

The `cargo xtask` checks shared by piqueld, sourceorigin and DirkEngine, so
each repository's xtask keeps only what is specific to it:

- `check-changed`: runs CI's quick checks for only what changed since the
  merge base with `origin/main`, picked from the path filters CI reads.
- `test-waits`: rejects sleeps in test code that don't say why they wait
  on the clock.
