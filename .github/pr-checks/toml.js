'use strict';

/**
 * Just enough TOML for the pull request checks, without a dependency: each
 * key with its table and raw value, so a check can compare a manifest before
 * and after a pull request. Values stay raw text; `string` and `items` read
 * the ones a check needs.
 */
class Toml {
  /** Reads `text`, which needn't be valid TOML: what can't be read is skipped. */
  constructor(text) {
    /** Each `key = value`: `{ table, key, value, line, section }`, where
     * `section` counts the table headers before it, so the entries of one
     * `[[array.table]]` element share it. Dotted keys stay joined: `a.b`. */
    this.entries = [];
    /** The line of each statement it couldn't read as a header or a
     * `key = value`, for a check that must fail closed on it. */
    this.unread = [];
    let table = '';
    let section = 0;
    for (const { text: statement, line } of Toml.statements(text)) {
      if (statement.startsWith('[')) {
        table = Toml.key(statement.replace(/^\[\[?|\]?\]$/g, ''));
        section += 1;
        continue;
      }
      const equals = Toml.find(statement, '=');
      if (equals < 0) {
        this.unread.push(line);
        continue;
      }
      this.entries.push({
        table,
        key: Toml.key(statement.slice(0, equals)),
        value: statement.slice(equals + 1).trim(),
        line,
        section,
      });
    }
  }

  /** The entries of `table`, such as `workspace.dependencies`. */
  table(name) {
    return this.entries.filter((entry) => entry.table === name);
  }

  /** The value of a string, or `null` for any other value. */
  static string(value) {
    for (const quote of ['"""', "'''", '"', "'"]) {
      if (value.length >= 2 * quote.length && value.startsWith(quote) && value.endsWith(quote)) {
        const inner = value.slice(quote.length, -quote.length);
        if (quote !== '"') return inner;
        try {
          return JSON.parse(value);
        } catch {
          return inner;
        }
      }
    }
    return null;
  }

  /** The elements of an array, or the `key = value` pairs of an inline
   * table, each as raw text with its spacing squeezed. */
  static items(value) {
    const items = [];
    let current = '';
    let depth = 0;
    for (const { c, quoted } of Toml.characters(value.trim().replace(/^[[{]|[\]}]$/g, ''))) {
      if (!quoted && (c === '[' || c === '{')) depth += 1;
      if (!quoted && (c === ']' || c === '}')) depth -= 1;
      if (!quoted && depth === 0 && c === ',') {
        items.push(current);
        current = '';
      } else {
        current += c;
      }
    }
    items.push(current);
    return items.map((item) => item.replace(/\s+/g, ' ').trim()).filter(Boolean);
  }

  /** A key or table name, its dotted parts trimmed and unquoted. */
  static key(text) {
    const parts = [];
    let current = '';
    for (const { c, quoted } of Toml.characters(text)) {
      if (!quoted && c === '.') {
        parts.push(current);
        current = '';
      } else {
        current += c;
      }
    }
    parts.push(current);
    return parts.map((part) => Toml.string(part.trim()) ?? part.trim()).join('.');
  }

  /** Where `char` first appears outside strings in `text`, or -1. */
  static find(text, char) {
    let index = 0;
    for (const { c, quoted } of Toml.characters(text)) {
      if (!quoted && c === char) return index;
      index += c.length;
    }
    return -1;
  }

  /** Each statement (a header or a key and its value, which may span lines
   * inside brackets), with comments dropped, and the line it starts on. */
  static *statements(text) {
    let current = '';
    let start = 1;
    let line = 1;
    let depth = 0;
    for (const { c, quoted } of Toml.characters(text)) {
      if (c === '\n') line += 1;
      if (!quoted && c === '\n' && depth <= 0) {
        if (current.trim()) yield { text: current.trim(), line: start };
        current = '';
        depth = 0;
        continue;
      }
      if (!quoted && (c === '[' || c === '{')) depth += 1;
      if (!quoted && (c === ']' || c === '}')) depth -= 1;
      if (!current.trim() && c.trim()) start = line;
      current += c === '\n' && !quoted ? ' ' : c;
    }
    if (current.trim()) yield { text: current.trim(), line: start };
  }

  /** Each character of `text` outside comments, and whether it sits in a
   * string. A newline ends a single-line string, so bad input can't swallow
   * the rest of the file. */
  static *characters(text) {
    let quote = null;
    for (let i = 0; i < text.length; i += 1) {
      const c = text[i];
      if (quote) {
        if (c === '\\' && quote[0] === '"' && i + 1 < text.length) {
          yield { c: c + text[i + 1], quoted: true };
          i += 1;
        } else if (text.startsWith(quote, i)) {
          yield { c: quote, quoted: true };
          i += quote.length - 1;
          quote = null;
        } else {
          if (c === '\n' && quote.length === 1) quote = null;
          yield { c, quoted: quote !== null };
        }
      } else if (c === '"' || c === "'") {
        quote = text.startsWith(c.repeat(3), i) ? c.repeat(3) : c;
        yield { c: quote, quoted: true };
        i += quote.length - 1;
      } else if (c === '#') {
        while (i + 1 < text.length && text[i + 1] !== '\n') i += 1;
      } else {
        yield { c, quoted: false };
      }
    }
  }
}

module.exports = { Toml };
