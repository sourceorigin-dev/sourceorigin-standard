'use strict';

const prBody = require('./pr-body.js');
const { Toml } = require('./toml.js');

/** The workspace's manifest, whose `[workspace.dependencies]` every crate
 * takes its dependencies from. */
const MANIFEST = 'Cargo.toml';

/** cargo-deny's configuration. */
const DENY = 'deny.toml';

/** The arrays of deny.toml whose elements are exceptions to its policy. */
const EXCEPTIONS = [
  'advisories.ignore',
  'licenses.allow',
  'licenses.exceptions',
  'bans.skip',
  'bans.skip-tree',
  'sources.allow-git',
  'sources.allow-registry',
];

/** The description's section that justifies a new dependency. */
const SECTION = 'Dependencies';

/**
 * DP8: a pull request that adds an entry to `[workspace.dependencies]` or
 * an exception to deny.toml says why in a "Dependencies" section of its
 * description, as the `tech-stack` skill writes it. What it says is left to
 * review.
 */
const newDependency = {
  name: 'new-dependency',

  async findings(pr) {
    const justified = prBody
      .sections(pr.body)
      .some(({ heading, text }) => heading.toLowerCase() === SECTION.toLowerCase() && text);
    if (justified) return [];
    const changed = new Set((await pr.files()).map(({ path }) => path));
    const added = [];
    for (const [path, read] of [
      [MANIFEST, newDependency.dependencies],
      [DENY, newDependency.exceptions],
    ]) {
      if (!changed.has(path)) continue;
      const before = new Set(read(new Toml((await pr.fileAt(pr.base, path)) ?? '')).keys());
      for (const [item, { line, what }] of read(new Toml((await pr.fileAt(pr.head, path)) ?? ''))) {
        if (!before.has(item)) added.push({ path, line, what });
      }
    }
    return added.map(({ path, line, what }) => ({
      path,
      line,
      message:
        `this adds ${what}: add a \`## ${SECTION}\` section to the description saying what it ` +
        `is for and what else was considered, as the \`tech-stack\` skill writes it.`,
    }));
  },

  /** Each workspace dependency of a manifest, by name. */
  dependencies(toml) {
    const found = new Map();
    for (const { table, key, line } of toml.entries) {
      const name =
        table === 'workspace.dependencies'
          ? key.split('.')[0]
          : table.startsWith('workspace.dependencies.') && table.split('.')[2];
      if (name && !found.has(name)) found.set(name, { line, what: `the workspace dependency \`${name}\`` });
    }
    return found;
  },

  /** Each exception of a deny.toml, by its array and its text, squeezed. */
  exceptions(toml) {
    const found = new Map();
    const sections = new Map();
    for (const { table, key, value, line, section } of toml.entries) {
      const array = table ? `${table}.${key}` : key;
      if (EXCEPTIONS.includes(array)) {
        for (const item of Toml.items(value)) {
          found.set(`${array}: ${item}`, { line, what: `\`${item}\` to \`${array}\`` });
        }
      } else if (EXCEPTIONS.includes(table)) {
        const element = sections.get(section) ?? { table, line, fields: [] };
        element.fields.push(`${key} = ${value.replace(/\s+/g, ' ')}`);
        sections.set(section, element);
      }
    }
    for (const { table, line, fields } of sections.values()) {
      const item = `{ ${fields.sort().join(', ')} }`;
      found.set(`${table}: ${item}`, { line, what: `\`${item}\` to \`${table}\`` });
    }
    return found;
  },
};

module.exports = newDependency;
