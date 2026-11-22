'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const newDependency = require('./new-dependency.js');
const { Fake } = require('./testing.js');

const manifest = (dependencies) =>
  `[workspace]\nmembers = ["crates/*"]\n\n[workspace.dependencies]\n${dependencies}\n\n[profile.release]\nlto = true\n`;

const findings = async (base, head, body = '') => {
  const changed = Object.keys(head).map((filename) => ({ filename, status: 'modified' }));
  return newDependency.findings(await Fake.pr({ body }, { files: { base, head }, changed }));
};

test('a new workspace dependency fails at its line, in either form', async () => {
  const before = manifest('serde = "1"');
  const after = manifest('serde = "1"\ntoml = { version = "1", features = [\n  "parse",\n] }') + '[workspace.dependencies.syn]\nversion = "2"\n';
  const result = await findings({ 'Cargo.toml': before }, { 'Cargo.toml': after });
  assert.deepEqual(
    result.map(({ path, line }) => `${path}:${line}`),
    ['Cargo.toml:6', 'Cargo.toml:13'],
  );
  assert.match(result[0].message, /^this adds the workspace dependency `toml`: add a `## Dependencies` section/);
});

test('changing an existing dependency, or anything else, passes', async () => {
  const before = manifest('serde = "1"\nfoo.version = "1"');
  const after = manifest('serde = { version = "1.1", features = ["derive"] }\nfoo.version = "2"') + '[lints]\nx = 1\n';
  assert.deepEqual(await findings({ 'Cargo.toml': before }, { 'Cargo.toml': after }), []);
});

test('a new workspace manifest counts every dependency as added', async () => {
  assert.equal((await findings({}, { 'Cargo.toml': manifest('a = "1"\nb = "1"') })).length, 2);
});

test('a new deny.toml exception fails, in an array or an array of tables', async () => {
  const before = '[advisories]\nignore = []\n[licenses]\nallow = ["MIT"]\n';
  const after =
    '[advisories]\nignore = [\n  "RUSTSEC-2024-0001", # why\n]\n[licenses]\nallow = ["MIT"]\n' +
    '[[licenses.exceptions]]\ncrate = "ring"\nallow = ["OpenSSL"]\n[bans]\nskip = [{ crate = "a@1", reason = "b" }]\n';
  const result = await findings({ 'deny.toml': before }, { 'deny.toml': after });
  assert.deepEqual(
    result.map(({ message }) => message.split(':')[0]),
    [
      'this adds `"RUSTSEC-2024-0001"` to `advisories.ignore`',
      'this adds `{ crate = "a@1", reason = "b" }` to `bans.skip`',
      'this adds `{ allow = ["OpenSSL"], crate = "ring" }` to `licenses.exceptions`',
    ],
  );
});

test('a Dependencies section in the description justifies them', async () => {
  const body = '## What and why\nx\n\n## Dependencies\n\n`toml` parses the manifests; `basic-toml` lacks spans.';
  const head = { 'Cargo.toml': manifest('toml = "1"') };
  assert.deepEqual(await findings({ 'Cargo.toml': manifest('') }, head, body), []);
  assert.equal((await findings({ 'Cargo.toml': manifest('') }, head, '## Dependencies\n<!-- why -->')).length, 1);
});

test('an unchanged manifest is not read', async () => {
  const pr = await Fake.pr({}, { files: { head: { 'Cargo.toml': manifest('a = "1"') } }, changed: [] });
  assert.deepEqual(await newDependency.findings(pr), []);
});
