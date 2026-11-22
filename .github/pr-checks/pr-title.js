'use strict';

/** The types a title may start with. */
const TYPES = ['feat', 'fix', 'refactor', 'docs', 'test', 'chore', 'ci', 'perf', 'build', 'revert'];

/**
 * G2: the title is a Conventional Commit, `type(scope)!: subject`, of one of
 * `TYPES` and the scopes the repository allows, since squash-merging makes it
 * the commit on `main`.
 */
const prTitle = {
  name: 'pr-title',
  TYPES,

  async findings(pr, settings) {
    const problem = prTitle.problem(pr.title, settings.scopes);
    if (!problem) return [];
    const scope = settings.scopes.length ? `(${settings.scopes[0]})` : '';
    const scopes = settings.scopes.length
      ? `one of ${settings.scopes.join(', ')}`
      : 'none, as this repository takes no scope';
    return [
      {
        message:
          `the title \`${pr.title}\` ${problem}. Write it as \`type(scope)!: subject\`: ` +
          `the type one of ${TYPES.join(', ')}; the scope, if any, ${scopes}; \`!\` for a ` +
          `breaking change; the subject saying what changes for the reader, such as ` +
          `\`fix${scope}: a negated glob no longer matches its own pattern\`.`,
      },
    ];
  },

  /** The title's parts, or `null` when it doesn't have the shape. */
  parse(title) {
    return (
      /^(?<type>\w+)(?:\((?<scope>[^()]*)\))?(?<breaking>!)?: (?<subject>\S.*)$/.exec(title)?.groups ??
      null
    );
  },

  /** What is wrong with the title, or `null`. */
  problem(title, scopes) {
    const parts = prTitle.parse(title);
    if (!parts) return 'is not `type(scope)!: subject`';
    if (!TYPES.includes(parts.type)) return `has the type \`${parts.type}\``;
    if (parts.scope !== undefined && !scopes.includes(parts.scope)) {
      return `has the scope \`${parts.scope}\``;
    }
    return null;
  },
};

module.exports = prTitle;
