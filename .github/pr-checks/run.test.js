'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { Runner } = require('./run.js');
const { Fake } = require('./testing.js');

const env = { CHECK: 'pr-title' };
/** The fake API of a pull request whose base has `standard`. */
const github = (pr, standard = '') => Fake.github(pr, { files: { base: { 'standard.toml': standard } } });

test('a finding fails the job with how to fix it and how to challenge the check', async () => {
  const core = Fake.core();
  await Runner.run({ ...github({ title: 'Add a thing' }), core }, env);
  assert.equal(core.errors.length, 1);
  assert.match(core.errors[0].message, /^\[pr-title\] the title `Add a thing`/);
  assert.match(core.errors[0].message, /ask the owner, then add `pr-title = "<why>"` under `\[pr-checks\.deviations\]`/);
  assert.match(core.failed, /pr-title found 1 problem/);
  assert.match(core.written, /### pr-title\n```\n\[pr-title\]/);
});

test('a passing check says so in the summary', async () => {
  const core = Fake.core();
  await Runner.run({ ...Fake.github(), core }, env);
  assert.deepEqual([core.errors, core.failed], [[], null]);
  assert.equal(core.written, '### pr-title\nPassed.\n');
});

test("the base's settings apply, not the head's", async () => {
  const core = Fake.core();
  const files = {
    base: { 'standard.toml': '[pr-checks]\nscopes = ["rules"]' },
    head: { 'standard.toml': '[pr-checks]\nscopes = ["web"]' },
  };
  await Runner.run({ ...Fake.github({ title: 'feat(web): a' }, { files }), core }, env);
  assert.match(core.errors[0].message, /has the scope `web`/);
});

test('a deviation turns findings into notices', async () => {
  const core = Fake.core();
  const standard = '[pr-checks.deviations]\npr-title = "titles come from an external tracker"';
  await Runner.run({ ...github({ title: 'x' }, standard), core }, env);
  assert.deepEqual([core.errors, core.failed, core.notices.length], [[], null, 1]);
  assert.match(core.written, /Allowed: the repository deviates from it: titles come/);
});

test('a finding with a place starts with it', () => {
  const text = Runner.format('lint-exceptions', { message: 'm', path: 'src/a.rs', line: 3 });
  assert.match(text, /^src\/a\.rs:3: \[lint-exceptions\] m\n {2}If the check/);
});

test("pr-checks.yml's matrix runs every check", () => {
  const workflow = fs.readFileSync(path.join(__dirname, '../workflows/pr-checks.yml'), 'utf8');
  const matrix = workflow.match(/^\s+check: \[(.*)\]$/m)[1].split(',').map((name) => name.trim());
  assert.deepEqual(matrix, Runner.CHECKS.map(({ name }) => name));
});
