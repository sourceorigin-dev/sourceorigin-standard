'use strict';

const { APPROVED } = require('./pull-request.js');
const { PATH, Settings } = require('./settings.js');
const { Toml } = require('./toml.js');

/** Files that set how the standard's rules apply, by path from any
 * directory, any change to which needs approval: they aren't parsed, so no
 * form of loosening slips by. Cargo's configuration can turn a lint off
 * through `rustflags`. */
const RULE_FILES = [PATH, 'clippy.toml', '.clippy.toml', 'deny.toml', '.cargo/config.toml', '.cargo/config'];

/** The repository's budgets, each equal to what it counts: each crate's
 * `#[expect]` lints and `// INVARIANT:` lines, and each manifest's lints set
 * to `allow` (`[allow]`), which the crate's `budget` rule counts in every
 * form TOML has. */
const BUDGET = 'budget.toml';

/** What a pull request does to a file, by its status in `pulls.listFiles`. */
const VERBS = { added: 'adds', removed: 'deletes', renamed: 'renames' };

/**
 * C11: a pull request that loosens a rule is always seen, and needs
 * approval: any change to a rule file, under its name or its old one, and
 * a budget of budget.toml that rises or appears. As every budget equals
 * its count, each new `#[expect]`, `// INVARIANT:` or lint a manifest
 * allows shows as one. The approval is the `approved-exception` label,
 * added by an approver after the latest push; report-only mode lists
 * without failing.
 */
const lintExceptions = {
  name: 'lint-exceptions',

  async findings(pr, settings) {
    const findings = [];
    for (const { path, status, previous } of await pr.files()) {
      const rule = lintExceptions.ruleFile(path) ?? (previous && lintExceptions.ruleFile(previous));
      if (rule) {
        findings.push({ path, message: `this ${VERBS[status] ?? 'changes'} ${rule}, which sets how the rules apply.` });
        if (path === PATH && status !== 'removed') new Settings(await pr.fileAt(pr.head, path), settings.known);
      } else if (path === BUDGET && status !== 'removed') {
        findings.push(...(await lintExceptions.raises(pr)));
      }
    }
    const approvers = settings.approvers.length ? settings.approvers.join(', ') : 'none yet';
    return findings.map((finding) => ({
      ...finding,
      message:
        `${finding.message} A loosened rule needs the \`${APPROVED}\` label, added after the ` +
        `latest push by an approver (\`approvers\` under \`[pr-checks]\` in ${PATH}: ${approvers}): ` +
        `ask one, and say in the exception's reason why the rule is wrong here.`,
    }));
  },

  async waiver(pr, settings) {
    if (!settings.enforceLintExceptions) return 'the repository runs this check in report-only mode';
    return pr.approval(settings.approvers);
  },

  /** The rule file `path` is, as `RULE_FILES` names it, or `undefined`. */
  ruleFile(path) {
    return RULE_FILES.find((file) => path === file || path.endsWith(`/${file}`));
  },

  /** The file at `path` in commit `sha`, read as TOML; none is empty. */
  async read(pr, sha, path) {
    return new Toml((await pr.fileAt(sha, path)) ?? '');
  },

  /** The budgets budget.toml raises or adds, or one finding when either
   * side holds anything but tables of whole numbers, which can't be
   * compared simply. */
  async raises(pr) {
    const before = lintExceptions.ceilings(await lintExceptions.read(pr, pr.base, BUDGET));
    const after = lintExceptions.ceilings(await lintExceptions.read(pr, pr.head, BUDGET));
    if (!before || !after) {
      return [{ path: BUDGET, message: 'this changes budget.toml in a form other than tables of whole numbers, which the check reads as a loosening.' }];
    }
    return [...after]
      .filter(([key, { ceiling }]) => !before.has(key) || ceiling > before.get(key).ceiling)
      .map(([key, { ceiling, line }]) => ({
        path: BUDGET,
        line,
        message: `this ${before.has(key) ? 'raises' : 'adds'} \`${key}\` to ${ceiling}.`,
      }));
  },

  /** Each budget of a budget.toml, by table and key, or `null` when it
   * holds anything else. */
  ceilings(toml) {
    const ceilings = new Map();
    for (const { table, key, value, line } of toml.entries) {
      if (!table || !/^\d+$/.test(value)) return null;
      ceilings.set(`${table}.${key}`, { ceiling: Number(value), line });
    }
    return ceilings;
  },
};

module.exports = lintExceptions;
