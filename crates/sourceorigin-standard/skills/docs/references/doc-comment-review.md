# Doc comment review

Review the doc comments a change adds or edits, before its pull request
opens. You weren't the agent who wrote them, and you have none of its
context. Judge them as a reader who has only the code would.

## Scope

Read only the `///` and `//!` lines the diff adds or changes:

```sh
git diff -U0 origin/main -- '*.rs'
```

Use the base branch your task gives instead of `origin/main` when it names
one. Read the code around a line only to check what the doc comment claims.
Don't judge other comments, other lines or the code itself.

## What to judge

Judge each doc comment against the doc comments entry of "What each kind of
documentation holds" in `.agents/skills/docs/SKILL.md`. Read it there, and
apply nothing else.

When a doc comment explains surprising behaviour of our own code, the code
is the problem, not the prose. Flag the symbol for a rename or a reshape
that makes the behaviour obvious without the comment, and delete the
explanation. Keep it only when the surprise comes from a dependency,
platform, vendor or protocol we can't change.

## Report

Edit nothing. For each finding, give:

- its `file:line`;
- **delete**, or **rewrite** with the new text;
- for an explanation of our own code, the symbol to rename or reshape, and
  how.

If every line passes, say so. The author applies the fixes before opening
the pull request.
