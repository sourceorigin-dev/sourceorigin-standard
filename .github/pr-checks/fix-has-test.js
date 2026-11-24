'use strict';

const prTitle = require('./pr-title.js');

/** Paths of test files: a `tests` or `test` directory, a sibling `tests.rs`,
 * and the `_test`, `.test` and `.spec` files of other languages. */
const TEST_PATH = /(^|\/)(tests?|__tests__)\/|(^|\/)tests\.rs$|[._-](test|spec)\.\w+$|(^|\/)test_\w+\.py$/;

/** An added line holding a test attribute, such as `#[test]` or
 * `#[tokio::test]`, for a test beside the code it tests. */
const TEST_ATTRIBUTE = /^\+\s*#\[(\w+::)*\w*test(_case)?\b/m;

/** An image in a description: Markdown, HTML, or a pasted attachment. */
const IMAGE = /!\[[^\]]*\]\([^)]+\)|<img\s|https:\/\/github\.com\/user-attachments\/assets\//i;

/** The label that marks a visual fix, proven by screenshots. */
const VISUAL = 'visual';

/**
 * MT8: a pull request whose title has the type `fix` changes a test, which
 * should fail without the fix, or carries the `visual` label and shows the
 * fix in images in its description.
 */
const fixHasTest = {
  name: 'fix-has-test',

  async findings(pr) {
    if (prTitle.parse(pr.title)?.type !== 'fix') return [];
    if (pr.labels.includes(VISUAL)) {
      if (IMAGE.test(pr.body)) return [];
      return [
        {
          message: `the \`${VISUAL}\` label says this fix is visual, but the description has no image: add screenshots from before and after the fix.`,
        },
      ];
    }
    for (const file of await pr.files()) {
      if (fixHasTest.isTest(file) || (await fixHasTest.editsInlineTests(pr, file))) return [];
    }
    return [
      {
        message:
          `this \`fix\` changes no test: add one that fails without the fix, in a \`tests.rs\`, ` +
          `a \`tests/\` directory or a \`#[test]\` beside the code. A visual fix carries the ` +
          `\`${VISUAL}\` label and screenshots from before and after in its description instead.`,
      },
    ];
  },

  /** Whether a change to a Rust file adds or edits a line inside one of
   * its inline test modules, `#[cfg(test)] mod … { … }`, at the head. */
  async editsInlineTests(pr, { path, status, patch }) {
    if (!path.endsWith('.rs') || status === 'removed' || !patch) return false;
    const modules = fixHasTest.testModules((await pr.fileAt(pr.head, path)) ?? '');
    return fixHasTest
      .added(patch)
      .some((line) => modules.some(([start, end]) => line >= start && line <= end));
  },

  /** The first and last lines of each inline test module of `text`, from
   * 1, its braces counted as written. */
  testModules(text) {
    const lines = text.split('\n');
    const modules = [];
    for (let i = 0; i < lines.length; i += 1) {
      if (!/^\s*#\[cfg\(test\)\]/.test(lines[i])) continue;
      let start = i + 1;
      while (start < lines.length && !lines[start].trim()) start += 1;
      if (!/\bmod\s+\w+\s*\{/.test(lines[start] ?? '')) continue;
      let depth = 0;
      for (let end = start; end < lines.length; end += 1) {
        depth += (lines[end].match(/\{/g) ?? []).length - (lines[end].match(/\}/g) ?? []).length;
        if (depth <= 0) {
          modules.push([start + 1, end + 1]);
          i = end;
          break;
        }
      }
    }
    return modules;
  },

  /** The lines a unified diff adds, by their line in the new file. */
  added(patch) {
    const lines = [];
    let line = 0;
    for (const text of patch.split('\n')) {
      const hunk = /^@@ -\d+(?:,\d+)? \+(\d+)/.exec(text);
      if (hunk) line = Number(hunk[1]);
      else if (text.startsWith('+')) lines.push(line++);
      else if (text.startsWith(' ')) line += 1;
    }
    return lines;
  },

  /** Whether a changed file is, or adds, a test. */
  isTest({ path, status, patch }) {
    if (status === 'removed') return false;
    return TEST_PATH.test(path) || TEST_ATTRIBUTE.test(patch ?? '');
  },
};

module.exports = fixHasTest;
