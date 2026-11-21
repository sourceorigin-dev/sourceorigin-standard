<!-- Title: a conventional commit that says what changes for whoever uses the
crate, such as `feat(check-changed): negated globs match as paths-filter's do`.
Keep the pull request to one change; stack dependent ones. -->

## What and why

<!-- What was wrong, what this changes, and the issue it closes ("Fixes #123"). -->

## Testing

<!-- How you tested it, including what you didn't test. -->

## Checklist

- [ ] The crate's README (its documentation on docs.rs) shows the change, and a repository's xtask can still follow it.
- [ ] A change to the public API is noted for the next release, with what each repository's xtask must change.
- [ ] `just check-changed` passes.
