'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const frozenFiles = require('./frozen-files.js');
const { Fake } = require('./testing.js');

const frozen = 'crates/*/migrations/*.sql';
const findings = async (changed, globs = `frozen-files = ["${frozen}"]`, head = {}) =>
  frozenFiles.findings(await Fake.pr({}, { changed, files: { head } }), Fake.settings(globs));
const file = (filename, status, previous_filename) => ({ filename, status, previous_filename });

test('modifying, deleting or renaming a frozen file fails at its old path', async () => {
  const result = await findings([
    file('crates/db/migrations/0001_init.sql', 'modified'),
    file('crates/db/migrations/0002_users.sql', 'removed'),
    file('crates/db/migrations/0004_users.sql', 'renamed', 'crates/db/migrations/0003_users.sql'),
  ]);
  assert.deepEqual(
    result.map(({ path, message }) => `${path} ${message.split(' (')[0]}`),
    [
      'crates/db/migrations/0001_init.sql this modifies a frozen file',
      'crates/db/migrations/0002_users.sql this deletes a frozen file',
      'crates/db/migrations/0003_users.sql this renames a frozen file',
    ],
  );
  assert.match(result[0].message, /put the change in a new file instead.*`approved-exception` label/);
});

test('adding a frozen file, or changing another, passes', async () => {
  const changed = [
    file('crates/db/migrations/0005_new.sql', 'added'),
    file('crates/db/src/lib.rs', 'modified'),
    file('crates/db/migrations/README.md', 'modified'),
    file('crates/db/migrations/0006.sql', 'renamed', 'crates/db/schema.sql'),
  ];
  const head = {
    'crates/db/migrations/0005_new.sql': '\n-- Adds the users table.\nCREATE TABLE users ();',
    'crates/db/migrations/0006.sql': '-- Moves the schema into a migration.\n',
  };
  assert.deepEqual(await findings(changed, undefined, head), []);
});

test('a new frozen file needs a header comment saying what it changes', async () => {
  const head = { 'crates/db/migrations/0005_new.sql': 'CREATE TABLE users ();\n-- too late' };
  const [finding] = await findings([file('crates/db/migrations/0005_new.sql', 'added')], undefined, head);
  assert.equal(`${finding.path}:${finding.line}`, 'crates/db/migrations/0005_new.sql:1');
  assert.match(finding.message, /^this adds a frozen file without a header: start it with a comment/);
  const empty = await findings([file('crates/db/migrations/0007.sql', 'added')], undefined, { 'crates/db/migrations/0007.sql': '--\n' });
  assert.equal(empty.length, 1);
});

test('a repository without frozen files passes', async () => {
  assert.deepEqual(await findings([file('crates/db/migrations/0001_init.sql', 'removed')], ''), []);
});

test('globs read as picomatch reads them', () => {
  const cases = [
    ['crates/*/migrations/*.sql', 'crates/db/migrations/1.sql', true],
    ['crates/*/migrations/*.sql', 'crates/db/x/migrations/1.sql', false],
    ['**/migrations/**', 'migrations/a/b.sql', true],
    ['**/migrations/**', 'crates/db/migrations/a.sql', true],
    ['snapshots/*.{json,ron}', 'snapshots/a.ron', true],
    ['snapshots/*.{json,ron}', 'snapshots/a.toml', false],
    ['data/[!x]?.bin', 'data/ab.bin', true],
    ['data/[!x]?.bin', 'data/xb.bin', false],
    ['.config/*.toml', '.config/a.toml', true],
    ['a.b', 'axb', false],
    ['{a,b}/c,d', 'a/c,d', true],
  ];
  for (const [glob, path, matches] of cases) {
    assert.equal(frozenFiles.pattern(glob).test(path), matches, `${glob} ${path}`);
  }
});

test('a deliberate change stands with the approval lint exceptions take', async () => {
  const settings = Fake.settings('approvers = ["owner"]');
  const events = [{ event: 'labeled', label: { name: 'approved-exception' }, actor: { login: 'Owner' }, created_at: '2026-11-23T10:01:00Z' }];
  const suites = [{ created_at: '2026-11-23T10:00:00Z', head_branch: 'feature' }];
  const labels = [{ name: 'approved-exception' }];
  assert.match(await frozenFiles.waiver(await Fake.pr({ labels }, { events, suites }), settings), /^Owner added the/);
  assert.equal(await frozenFiles.waiver(await Fake.pr(), settings), undefined);
});
