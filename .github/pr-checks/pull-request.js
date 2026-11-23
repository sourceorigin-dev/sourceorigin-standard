'use strict';

/** The label that approves the exceptions a pull request makes. */
const APPROVED = 'approved-exception';

/**
 * The pull request a check runs on, read through the GitHub API rather than
 * a checkout. Tests build one from plain values with `new PullRequest(...)`.
 */
class PullRequest {
  /** `pr` holds what `pulls.get` returns that the checks read. */
  constructor(pr, { github = null, repo = null } = {}) {
    this.number = pr.number;
    this.title = pr.title;
    this.body = pr.body ?? '';
    this.labels = (pr.labels ?? []).map((label) => label.name);
    this.base = pr.base?.sha;
    this.head = pr.head?.sha;
    /** The branch it merges from, and the owner and name of the
     * repository holding it: a fork's for a pull request from one. */
    this.headRef = pr.head?.ref;
    this.headOwner = pr.head?.repo?.owner?.login;
    this.headName = pr.head?.repo?.name;
    /** How many files GitHub says it changes. */
    this.changedFiles = pr.changed_files;
    this.github = github;
    this.repo = repo;
  }

  /** Fetches the pull request the event is about, as it is now: a re-run
   * replays the event's payload, which may hold an old title or body. */
  static async load({ github, context }) {
    const { data } = await github.rest.pulls.get({
      ...context.repo,
      pull_number: context.payload.pull_request.number,
    });
    return new PullRequest(data, { github, repo: context.repo });
  }

  /** Each commit's SHA and message, oldest first. */
  async commits() {
    const commits = await this.github.paginate(this.github.rest.pulls.listCommits, {
      ...this.repo,
      pull_number: this.number,
      per_page: 100,
    });
    return commits.map(({ sha, commit }) => ({ sha, message: commit.message }));
  }

  /** Each file it changes: `{ path, status, patch, previous }`, where
   * `patch` is the unified diff, or `undefined` when GitHub leaves out a
   * large one, and `previous` a renamed file's old path. Throws when GitHub
   * lists fewer files than it changes, as `listFiles` stops at 3000, so no
   * check passes on a part of them. */
  async files() {
    const files = await this.github.paginate(this.github.rest.pulls.listFiles, {
      ...this.repo,
      pull_number: this.number,
      per_page: 100,
    });
    if (files.length < (this.changedFiles ?? 0)) {
      throw new Error(
        `GitHub lists ${files.length} of the ${this.changedFiles} files this pull request ` +
          'changes, so the checks cannot see them all: split it into smaller pull requests.',
      );
    }
    return files.map(({ filename, status, patch, previous_filename: previous }) => ({
      path: filename,
      status,
      patch,
      previous,
    }));
  }

  /** Why the exceptions it makes may stand: the `approved-exception`
   * label, added by one of `approvers` (in any case) after its head commit
   * was last pushed, so a later push needs it added again. `undefined`
   * without one. The push time is the newest entry of the head
   * repository's activity log for the head branch, which no workflow or
   * user can skip or delete: a push, fast-forward, force-push or the
   * branch's creation, even while the pull request conflicts. It must have
   * moved the branch to the head commit; otherwise, or with no entry, or a
   * log the token can't read, there is no approval. So a log that lags
   * behind the branch fails closed. */
  async approval(approvers) {
    if (!this.labels.includes(APPROVED)) return undefined;
    const events = await this.github.paginate(this.github.rest.issues.listEvents, {
      ...this.repo,
      issue_number: this.number,
      per_page: 100,
    });
    const labeled = events.filter(({ event, label }) => event === 'labeled' && label?.name === APPROVED).at(-1);
    const login = labeled?.actor?.login ?? '';
    if (!approvers.some((approver) => approver.toLowerCase() === login.toLowerCase())) return undefined;
    let latest;
    try {
      const { data } = await this.github.rest.repos.listActivities({
        owner: this.headOwner,
        repo: this.headName,
        ref: `refs/heads/${this.headRef}`,
        per_page: 1,
      });
      [latest] = data;
    } catch (error) {
      if (error.status === 403 || error.status === 404) return undefined;
      throw error;
    }
    if (latest?.after !== this.head) return undefined;
    if (!(Date.parse(labeled.created_at) > Date.parse(latest.timestamp))) return undefined;
    return `${login} added the \`${APPROVED}\` label after the latest push`;
  }

  /** The text of the file at `path` in commit `sha`, or `null` if it has
   * none. */
  async fileAt(sha, path) {
    try {
      const { data } = await this.github.rest.repos.getContent({
        ...this.repo,
        path,
        ref: sha,
        mediaType: { format: 'raw' },
      });
      return data;
    } catch (error) {
      if (error.status === 404) return null;
      throw error;
    }
  }
}

module.exports = { PullRequest, APPROVED };
