'use strict';

const { APPROVED } = require('./pull-request.js');
const { PATH, Settings } = require('./settings.js');
const { Toml } = require('./toml.js');

/** Files that set how the standard's rules apply, by path from any
 * directory, any change to which needs approval: they aren't parsed, so no
 * form of loosening slips by. Cargo's configuration can turn a lint off
 * through `rustflags`, and `dylint.toml` configures the Dylint lints. */
const RULE_FILES = [PATH, 'clippy.toml', '.clippy.toml', 'deny.toml', 'dylint.toml', '.cargo/config.toml', '.cargo/config'];

/** The repository's budgets, each equal to what it counts: each crate's
 * `#[expect]` lints, `// INVARIANT:` lines and `// wall-clock:` lines, and
 * each manifest's lints set to `allow` (`[allow]`), which the crate's `budget` rule counts in every
 * form TOML has. */
const BUDGET = 'budget.toml';

/** The lists that scope a deviation of standard.toml, which a change may
 * only shorten without approval. */
const SCOPES = ['paths', 'crates'];

/** What a pull request does to a file, by its status in `pulls.listFiles`. */
const VERBS = { added: 'adds', removed: 'deletes', renamed: 'renames' };

/**
 * C11: a pull request that loosens a rule is always seen, and needs
 * approval: any change to a rule file, under its name or its old one, but
 * one to standard.toml that only tightens it, and a budget of budget.toml
 * that rises or appears. As every budget equals
 * its count, each new `#[expect]`, `// INVARIANT:`, `// wall-clock:` or lint
 * a manifest allows shows as one. The approval is the `approved-exception` label,
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
        const tightens = path === PATH && status === 'modified' && (await lintExceptions.tightens(pr));
        if (!tightens) {
          findings.push({ path, message: `this ${VERBS[status] ?? 'changes'} ${rule}, which sets how the rules apply.` });
        }
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

  /** Whether the pull request's change to standard.toml only tightens it:
   * it removes deviations, removes entries from a deviation's `paths` or
   * `crates`, or rewords a reason, and changes nothing else. A side that
   * can't be read as standard.toml loosens it. */
  async tightens(pr) {
    const before = lintExceptions.standard(await lintExceptions.read(pr, pr.base, PATH));
    const after = lintExceptions.standard(await lintExceptions.read(pr, pr.head, PATH));
    if (!before || !after) return false;
    const settings = [...new Set([...before.settings.keys(), ...after.settings.keys()])];
    if (settings.some((key) => before.settings.get(key) !== after.settings.get(key))) return false;
    return [...after.deviations].every(([rule, scope]) => {
      if (!before.deviations.has(rule)) return false;
      const was = before.deviations.get(rule);
      if (scope === null || was === null) return scope === was;
      return SCOPES.every((list) => [...scope[list]].every((entry) => was[list].has(entry)));
    });
  },

  /** A standard.toml as its deviations, each by its table and rule with
   * its scope (`null` for everywhere, else a set of each of `SCOPES`), and
   * every other setting by its dotted key with its value as written; or
   * `null` when it holds anything else. Reasons don't count. */
  standard(toml) {
    if (toml.unread.length) return null;
    const settings = new Map();
    const written = new Map();
    const entries = toml.entries.map(({ table, key, value }) => [table ? `${table}.${key}` : key, value]);
    for (let i = 0; i < entries.length; i += 1) {
      const [key, value] = entries[i];
      if (value.startsWith('{')) {
        for (const item of Toml.items(value)) {
          const equals = Toml.find(item, '=');
          if (equals < 0) return null;
          entries.push([`${key}.${Toml.key(item.slice(0, equals))}`, item.slice(equals + 1).trim()]);
        }
        continue;
      }
      const deviation = /^((?:pr-checks\.)?deviations)\.([^.]+)(?:\.(reason|paths|crates))?$/.exec(key);
      if (!deviation) {
        if (/^(pr-checks\.)?deviations(\.|$)/.test(key) || settings.has(key)) return null;
        settings.set(key, value.replace(/\s+/g, ' '));
        continue;
      }
      const [, table, rule, field = 'everywhere'] = deviation;
      const fields = written.get(`${table}.${rule}`) ?? {};
      const read = SCOPES.includes(field) ? Settings.read('strings', value) : Toml.string(value);
      if (read === null || field in fields || (table !== 'deviations' && field !== 'everywhere')) return null;
      fields[field] = read;
      written.set(`${table}.${rule}`, fields);
    }
    const deviations = new Map();
    for (const [rule, fields] of written) {
      if ('everywhere' in fields && Object.keys(fields).length > 1) return null;
      const scoped = SCOPES.some((list) => fields[list]);
      deviations.set(rule, scoped ? Object.fromEntries(SCOPES.map((list) => [list, new Set(fields[list] ?? [])])) : null);
    }
    return { settings, deviations };
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
