'use strict';

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

module.exports = { PullRequest };
