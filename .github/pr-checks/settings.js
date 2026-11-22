'use strict';

const { Toml } = require('./toml.js');

/** Where the settings live: the `[pr-checks]` table of the repository's
 * standard.toml. */
const PATH = 'standard.toml';

/**
 * How a repository applies the pull request checks: the `[pr-checks]`
 * table of its standard.toml, read at the pull request's base, so a pull
 * request can't loosen the checks it is checked by. `load` reads it
 * through the API; tests build one from text.
 */
class Settings {
  /** Each key of `[pr-checks]`, with how its value reads and its default.
   * Its property is its name in camel case: `issue-prefix`, `issuePrefix`. */
  static KEYS = {
    scopes: { read: 'strings', default: [] },
  };

  /** Reads `text`, a standard.toml or `null` for none; `known` are the
   * check names a deviation may name. Throws on a key or value it can't
   * read, or a form of `pr-checks` it doesn't, naming its line. */
  constructor(text, known) {
    for (const [key, { default: value }] of Object.entries(Settings.KEYS)) {
      this[Settings.property(key)] = value;
    }
    /** Each check the repository departs from, with its reason. */
    this.deviations = new Map();
    for (const { table, key, value, line } of new Toml(text ?? '').entries) {
      const fail = (expected) => {
        throw new Error(`${PATH}:${line}: \`${key}\` of \`[${table}]\` must be ${expected}`);
      };
      const path = table ? `${table}.${key}` : key;
      if (path !== 'pr-checks' && !path.startsWith('pr-checks.')) continue;
      if (key.includes('.') || !['pr-checks', 'pr-checks.deviations'].includes(table)) {
        throw new Error(
          `${PATH}:${line}: write \`${path}\` as a key under a \`[pr-checks]\` or ` +
            '`[pr-checks.deviations]` header, which the checks read',
        );
      }
      if (table === 'pr-checks') {
        const spec = Settings.KEYS[key];
        if (!spec) fail(`one of ${Object.keys(Settings.KEYS).join(', ')}, or the \`deviations\` table`);
        const read = Settings.read(spec.read, value);
        if (read === null) fail(spec.read === 'strings' ? 'an array of strings' : `a ${spec.read}`);
        this[Settings.property(key)] = read;
      } else if (table === 'pr-checks.deviations') {
        const reason = Toml.string(value);
        if (!known.includes(key) || !reason?.trim()) {
          fail(`one of ${known.join(', ')}, given as \`${key} = "<why the repository departs from it>"\``);
        }
        this.deviations.set(key, reason);
      }
    }
  }

  /** The settings at the pull request's base commit. */
  static async load(pr, known) {
    return new Settings(await pr.fileAt(pr.base, PATH), known);
  }

  /** A value as `kind` reads it, or `null` when it isn't one. */
  static read(kind, value) {
    if (kind === 'string') return Toml.string(value);
    if (kind === 'boolean') return value === 'true' ? true : value === 'false' ? false : null;
    if (!value.startsWith('[')) return null;
    const items = Toml.items(value).map((item) => Toml.string(item));
    return items.includes(null) ? null : items;
  }

  /** A key's property: its name in camel case. */
  static property(key) {
    return key.replace(/-(\w)/g, (_, letter) => letter.toUpperCase());
  }
}

module.exports = { Settings, PATH };
