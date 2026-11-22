'use strict';

const { PullRequest } = require('./pull-request.js');
const { Settings } = require('./settings.js');

/**
 * Runs one check from pr-checks.yml's `actions/github-script` step:
 * `await Runner.run({ github, context, core })`.
 */
class Runner {
  /** Every check, each a job of pr-checks.yml's matrix: an object with a
   * `name`, `findings(pr, settings)` and, optionally, `waiver(pr, settings)`,
   * the reason findings shouldn't fail the job, or nothing. */
  static CHECKS = [
    require('./pr-title.js'),
    require('./pr-body.js'),
    require('./no-co-authors.js'),
  ];

  /** Runs the check named in `CHECK`, with the settings of the pull
   * request's base. Each
   * finding becomes an error annotation and a line of the job summary, and
   * fails the job, unless the repository deviates from the check or the
   * check waives it: then it is a notice, still in the summary. */
  static async run({ github, context, core }, env = process.env) {
    const check = Runner.CHECKS.find(({ name }) => name === env.CHECK);
    if (!check) throw new Error(`no check is named \`${env.CHECK}\``);
    const pr = await PullRequest.load({ github, context });
    const settings = await Settings.load(pr, Runner.CHECKS.map(({ name }) => name));
    const findings = await check.findings(pr, settings);
    const deviation = settings.deviations.get(check.name);
    const waiver = deviation
      ? `the repository deviates from it: ${deviation}`
      : await check.waiver?.(pr, settings);

    const lines = findings.map((finding) => Runner.format(check.name, finding));
    findings.forEach(({ path, line }, index) => {
      const properties = { title: check.name, file: path, startLine: line };
      if (waiver) core.notice(lines[index], properties);
      else core.error(lines[index], properties);
    });
    core.summary.addHeading(check.name, 3);
    if (findings.length === 0) core.summary.addRaw('Passed.', true);
    else core.summary.addCodeBlock(lines.join('\n\n'));
    if (waiver && findings.length) core.summary.addRaw(`Allowed: ${waiver}.`, true);
    await core.summary.write();
    if (findings.length && !waiver) {
      core.setFailed(`${check.name} found ${findings.length} problem(s); each says how to fix it.`);
    }
  }

  /** A finding as the crate prints a violation: where, the check and what
   * to fix, then how to challenge the check. */
  static format(name, { message, path, line }) {
    const where = path ? `${path}${line ? `:${line}` : ''}: ` : '';
    return (
      `${where}[${name}] ${message}\n  If the check is wrong here, ask the owner, then add ` +
      `\`${name} = "<why>"\` under \`[pr-checks.deviations]\` in standard.toml for this ` +
      `repository, or change the check in sourceorigin-standard for all.`
    );
  }
}

module.exports = { Runner };
