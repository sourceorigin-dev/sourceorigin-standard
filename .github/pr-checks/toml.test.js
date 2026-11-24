'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { Toml } = require('./toml.js');

test('entries carry their table, line and section, comments dropped', () => {
  const toml = new Toml(
    ['top = 1 # a comment', '', '[workspace.dependencies]', 'serde = "1" # why', 'a.b = 2'].join('\n'),
  );
  assert.deepEqual(toml.entries, [
    { table: '', key: 'top', value: '1', line: 1, section: 0 },
    { table: 'workspace.dependencies', key: 'serde', value: '"1"', line: 4, section: 1 },
    { table: 'workspace.dependencies', key: 'a.b', value: '2', line: 5, section: 1 },
  ]);
});

test('a value spanning lines inside brackets is one entry', () => {
  const toml = new Toml('ignore = [\n  "A", # first\n  { id = "B", reason = "x]" },\n]\nnext = 1');
  assert.equal(toml.entries.length, 2);
  assert.deepEqual(Toml.items(toml.entries[0].value), ['"A"', '{ id = "B", reason = "x]" }']);
  assert.equal(toml.entries[1].line, 5);
});

test('a comment sign inside a string is text', () => {
  assert.equal(new Toml('a = "x # y"').entries[0].value, '"x # y"');
});

test('quoted keys and array table headers read as their names', () => {
  const toml = new Toml('[[licenses.exceptions]]\n"my.crate" = 1\n[[licenses.exceptions]]\nb = 2');
  assert.deepEqual(
    toml.entries.map(({ table, key, section }) => [table, key, section]),
    [
      ['licenses.exceptions', 'my.crate', 1],
      ['licenses.exceptions', 'b', 2],
    ],
  );
});

test('strings read whatever their quotes', () => {
  assert.equal(Toml.string('"a\\"b"'), 'a"b');
  assert.equal(Toml.string("'a\\b'"), 'a\\b');
  assert.equal(Toml.string('"""a\nb"""'), 'a\nb');
  assert.equal(Toml.string('12'), null);
});
