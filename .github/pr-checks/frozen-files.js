'use strict';

const { APPROVED } = require('./pull-request.js');

/** What a pull request does to a file that exists on the base branch, by
 * the status `pulls.listFiles` gives it. */
const CHANGES = { modified: 'modifies', changed: 'changes the mode of', removed: 'deletes', renamed: 'renames' };

/** A first line that is a comment with text, in the syntax of SQL, shell,
 * TOML, C-like languages or HTML. */
const HEADER = /^\s*(--|#|\/\/|\/\*|<!--|;)\s*\S/;

/**
 * PQ2: a file matching a glob of `frozen-files` under `[pr-checks]` that
 * exists on the base branch, such as an applied migration, is not modified,
 * renamed or deleted, and a new one starts with a comment saying what it
 * changes. A deliberate change needs the same approval as a lint exception.
 * Only a pull request can check it: on `main` there is nothing to compare.
 */
const frozenFiles = {
  name: 'frozen-files',

  async findings(pr, settings) {
    const globs = settings.frozenFiles.map((glob) => [glob, frozenFiles.pattern(glob)]);
    const frozen = (path) => globs.find(([, pattern]) => pattern.test(path))?.[0];
    const findings = [];
    for (const { path, previous, status } of await pr.files()) {
      const before = previous ?? path;
      const glob = frozen(before);
      if (CHANGES[status] && glob) {
        findings.push({
          path: before,
          message:
            `this ${CHANGES[status]} a frozen file (\`${glob}\` under \`[pr-checks] frozen-files\`), ` +
            `which never changes once on the base branch: put the change in a new file instead, such ` +
            `as a new migration. A deliberate change needs the \`${APPROVED}\` label, added after ` +
            `the latest push by an approver.`,
        });
      }
      const created = status === 'added' || status === 'copied' || (status === 'renamed' && !glob);
      if (created && frozen(path)) {
        const first = ((await pr.fileAt(pr.head, path)) ?? '').split('\n').find((line) => line.trim());
        if (!HEADER.test(first ?? '')) {
          findings.push({
            path,
            line: 1,
            message:
              `this adds a frozen file without a header: start it with a comment saying what it ` +
              `changes, such as \`-- Adds the users table and its email index.\`, as it can't be ` +
              `explained later.`,
          });
        }
      }
    }
    return findings;
  },

  async waiver(pr, settings) {
    return pr.approval(settings.approvers);
  },

  /** A glob as a regular expression, as picomatch reads it with dotfiles:
   * `**` spans directories, `*` and `?` stay within one, `{a,b}` is either
   * and `[…]` a class. */
  pattern(glob) {
    let source = '';
    let braces = 0;
    for (let i = 0; i < glob.length; i += 1) {
      const c = glob[i];
      if (glob.startsWith('**/', i)) {
        source += '(?:.*/)?';
        i += 2;
      } else if (glob.startsWith('**', i)) {
        source += '.*';
        i += 1;
      } else if (c === '*') source += '[^/]*';
      else if (c === '?') source += '[^/]';
      else if (c === '{') {
        source += '(?:';
        braces += 1;
      } else if (c === '}' && braces > 0) {
        source += ')';
        braces -= 1;
      } else if (c === ',' && braces > 0) source += '|';
      else if (c === '[') {
        const end = glob.indexOf(']', i + 1);
        source += end < 0 ? '\\[' : glob.slice(i, end + 1).replace(/^\[!/, '[^');
        if (end >= 0) i = end;
      } else source += c.replace(/[.+^$()|\\]/g, '\\$&');
    }
    return new RegExp(`^${source}$`);
  },
};

module.exports = frozenFiles;
