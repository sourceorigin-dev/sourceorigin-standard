'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const prBody = require('./pr-body.js');
const { Fake } = require('./testing.js');

const template = [
  '<!-- Title: a conventional commit. -->',
  '',
  '## What and why',
  '',
  '<!-- What was wrong, and the issue it closes ("Fixes #123"). -->',
  '',
  '## Testing',
  '',
  '<!-- How you tested it. -->',
].join('\n');

const findings = async (body, settings = '', files = { base: { '.github/pull_request_template.md': template } }) =>
  prBody.findings(await Fake.pr({ body }, { files }), Fake.settings(settings));

test('a description filling every heading passes', async () => {
  assert.deepEqual(await findings('## What and why\nIt broke.\n\n## Testing\n\nRan it.'), []);
});

test('a heading left empty or with only the prompt fails', async () => {
  const messages = (await findings(`## What and why\nIt broke.\n${template.split('## Testing')[1]}`)).map(
    (finding) => finding.message,
  );
  assert.equal(messages.length, 1);
  assert.match(messages[0], /nothing under `Testing`/);
  assert.equal((await findings(template)).length, 2);
});

test('a missing heading fails, whatever the case of those present', async () => {
  const [finding] = await findings('## what AND why\nIt broke.');
  assert.match(finding.message, /nothing under `Testing`: fill in every heading/);
});

test("a subsection counts as its heading's text, and a fenced heading as none", () => {
  assert.deepEqual(prBody.missing(template, '## What and why\n### Detail\n## Testing\nRan it.'), []);
  assert.deepEqual(prBody.missing(template, '## What and why\n```\n## Testing\nx\n```'), ['Testing']);
});

test('a repository without a template is told to add one', async () => {
  const [finding] = await findings('anything', '', {});
  assert.equal(finding.path, '.github/pull_request_template.md');
});

test("an issue link is optional, and one in the repository's form passes", () => {
  assert.deepEqual(prBody.wrongLinks('No issue.', '#'), []);
  assert.deepEqual(prBody.wrongLinks('Fixes #12. Part of #13. fixes #14', '#'), []);
  assert.deepEqual(prBody.wrongLinks('Fixes PQD-123', 'PQD-'), []);
});

test('a closing link in another form fails', () => {
  assert.deepEqual(prBody.wrongLinks('Closes #12, resolves:  #13', '#'), ['Closes #12', 'resolves: #13']);
  assert.deepEqual(prBody.wrongLinks('Fixes #12', 'PQD-'), ['Fixes #12']);
  assert.deepEqual(
    prBody.wrongLinks('Closes https://github.com/o/r/issues/3', '#'),
    ['Closes https://github.com/o/r/issues/3'],
  );
  assert.deepEqual(prBody.wrongLinks('Fixes owner/repo#12', '#'), ['Fixes owner/repo#12']);
  assert.deepEqual(prBody.wrongLinks('<!-- Closes #12 -->', '#'), []);
});

test("words that only look like issues aren't links", () => {
  assert.deepEqual(prBody.wrongLinks('This fixes UTF-8 paths and resolves CVE-2024-1234.', 'PQD-'), []);
  assert.deepEqual(prBody.wrongLinks('Fixes PQD-12, fixes #12a', '#'), []);
});

test('a link in code closes nothing, so it passes', () => {
  assert.deepEqual(prBody.wrongLinks('Reads `Closes #12` as wrong.', '#'), []);
  assert.deepEqual(prBody.wrongLinks('```\nCloses #12\n```\nThen Closes #13', '#'), ['Closes #13']);
});

test('the finding for a link gives the form to use', async () => {
  const body = '## What and why\nCloses #4\n## Testing\nRan it.';
  const [finding] = await findings(body, 'issue-prefix = "PQD-"');
  assert.match(finding.message, /^`Closes #4` closes an issue .* write `Fixes PQD-<number>`/);
});
