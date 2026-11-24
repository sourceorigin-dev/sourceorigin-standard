'use strict';

const { PullRequest } = require('./pull-request.js');
const { Settings } = require('./settings.js');

/**
 * Stand-ins for what `actions/github-script` hands a script, for the tests:
 * `Fake.core()` records what a check reports, and `Fake.github(pr)` answers
 * the API calls with `pr`'s values.
 */
class Fake {
  /** A `core` recording annotations, the job summary and the failure. */
  static core() {
    const core = { errors: [], notices: [], failed: null, written: '' };
    let summary = '';
    core.error = (message, properties) => core.errors.push({ message, ...properties });
    core.notice = (message, properties) => core.notices.push({ message, ...properties });
    core.setFailed = (message) => {
      core.failed = message;
    };
    core.summary = {
      addHeading: (text) => ((summary += `### ${text}\n`), core.summary),
      addRaw: (text) => ((summary += `${text}\n`), core.summary),
      addCodeBlock: (text) => ((summary += `\`\`\`\n${text}\n\`\`\`\n`), core.summary),
      write: async () => {
        core.written = summary;
      },
    };
    return core;
  }

  /** A `github` and `context` for pull request number 1, from base commit
   * `base` to head commit `head`, with the fields of `pr` over a passing
   * default, `files[sha][path]` the text of each file, `commits` its
   * commits, as `{ sha, commit: { message } }`, `changed` the files it
   * changes, as `{ filename, status, patch }`, `events` its issue events,
   * as `{ event, label: { name }, actor: { login }, created_at }`, and
   * `suites` the check suites of its head, as `{ created_at }`. */
  static github(pr = {}, { files = {}, commits = [], changed = [], events = [], suites = [] } = {}) {
    const data = {
      number: 1,
      title: 'feat: a change',
      body: '',
      labels: [],
      base: { sha: 'base' },
      head: { sha: 'head' },
      ...pr,
    };
    const getContent = async ({ ref, path }) => {
      const text = files[ref]?.[path];
      if (text === undefined) throw Object.assign(new Error('Not Found'), { status: 404 });
      return { data: text };
    };
    return {
      github: {
        rest: {
          pulls: {
            get: async () => ({ data }),
            listCommits: async () => ({ data: commits }),
            listFiles: async () => ({ data: changed }),
          },
          repos: { getContent },
          issues: { listEvents: async () => ({ data: events }) },
          checks: { listSuitesForRef: async () => ({ data: suites }) },
        },
        paginate: async (method, parameters) => (await method(parameters)).data,
      },
      context: { repo: { owner: 'o', repo: 'r' }, payload: { pull_request: { number: 1 } } },
    };
  }

  /** The pull request `Fake.github` describes, as a check receives it. */
  static async pr(pr = {}, options = {}) {
    return PullRequest.load(Fake.github(pr, options));
  }

  /** Settings from `toml`, the lines of a `[pr-checks]` table, as a
   * check receives them. */
  static settings(toml = '') {
    return new Settings(`[pr-checks]\n${toml}`, []);
  }
}

module.exports = { Fake };
