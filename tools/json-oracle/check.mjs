// treebank json oracle: V8's JSON.parse, one long-lived process.
//
// Reads one absolute path per line on stdin, writes "<path>\tvalid|invalid"
// per line on stdout. Judges each file on its own text: JSON has no imports,
// no configuration and no project context to be missing, so there is nothing
// here to disable.
//
// V8 and not serde_json, which was measured against it and is NOT a
// conformant reference: serde_json rejects nesting deeper than 128 (its
// default recursion limit — 127 valid, 128 invalid, measured) and rejects
// lone-surrogate escapes like "\ud800" that RFC 8259 permits. Both are
// over-strict, which is the dangerous direction: a valid file called invalid
// is booked as corpus noise and a real grammar gap disappears silently.
// CPython's json agrees with V8 on everything measured but raises
// RecursionError (not ValueError) at ~100k nesting depth, where V8's parser
// is iterative and has no depth limit at all. See ledger.json's
// oracle_not_serde_json.
//
// Two positions this file takes, because JSON's spec leaves them open:
//
//   1. A leading UTF-8 BOM makes a file INVALID. RFC 8259 s8.1 says
//      implementations MAY ignore one; V8's JSON.parse does not, and this
//      oracle is V8 rather than V8-plus-a-courtesy. Hence ignoreBOM: true,
//      which keeps the U+FEFF in the decoded text — the DEFAULT TextDecoder
//      strips it, so accepting a BOM is what you get by accident here, and
//      that is worth one line of vigilance. Measured incidence in the corpus
//      this ledger's numbers come from: 0 of 1426 files.
//   2. JSON text MUST be UTF-8 (RFC 8259 s8.1), so a decode failure is a
//      REJECT rather than an I/O error: the bytes were read fine, they are
//      just not JSON text. fatal: true makes that explicit rather than
//      letting U+FFFD replacement characters turn undecodable bytes into a
//      well-formed string.
//
// An unreadable file is NOT an invalid file: anything that stops this
// process from getting a file's bytes exits non-zero without emitting a
// verdict, because validate() only ever runs on files the grammar already
// failed and an `invalid` verdict books the file as noise. A mistyped corpus
// root that scored every file invalid would drive gap_files to zero and
// report a flawless grammar.

import { readFileSync } from 'node:fs';
import { createInterface } from 'node:readline';

// Keep the BOM (see position 1) and refuse undecodable bytes (position 2).
const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });

function isValid(bytes) {
  let text;
  try {
    text = decoder.decode(bytes);
  } catch {
    return false; // not UTF-8, so not JSON text
  }
  try {
    JSON.parse(text);
    return true;
  } catch {
    return false;
  }
}

// The engine is the oracle, so the engine is what has to be checked — a
// version string would only tell us which node is on PATH, not whether its
// JSON.parse still draws the line where this ledger's verdicts assume. These
// are the cases that separate strict JSON from the dialects it is mistaken
// for and from the parsers that are laxer or stricter than the spec; if any
// of them moves, every sweep number produced by this oracle is void, so the
// oracle refuses to produce more of them. Costs ~0.1 ms once per batch.
const SELFTEST = [
  ['{"a": 1}', true],
  ['"top-level string"', true],
  ['{"a": 1, "a": 2}', true],          // duplicate keys: SHOULD be unique, not MUST
  ['{"a": "\\ud800"}', true],          // lone surrogate escape: permitted (serde_json rejects)
  ['[' .repeat(200) + ']'.repeat(200), true],  // depth 200 (serde_json rejects >127)
  ['{"a": 1,}', false],                // JSONC/JSON5 trailing comma
  ['{"a": 1} // c', false],            // JSONC comment
  ["{'a': 1}", false],                 // JSON5 single quotes
  ['{a: 1}', false],                   // JSON5 unquoted key
  ['{"a": NaN}', false],               // JSON5 / python json.loads default
  ['{"a": 01}', false],
  ['{"a": "\\u12"}', false],           // short \u escape (the grammar accepts this)
  ['', false],                         // empty document (the grammar accepts this)
  ['{"a": 1} {"b": 2}', false],        // two documents (the grammar accepts this)
  ['﻿{"a": 1}', false],           // BOM: position 1 above
];

for (const [text, want] of SELFTEST) {
  let got;
  try {
    JSON.parse(text);
    got = true;
  } catch {
    got = false;
  }
  if (got !== want) {
    const shown = text.length > 40 ? `${text.slice(0, 40)}…` : text;
    console.error(
      `json oracle: SELF-TEST FAILED on ${JSON.stringify(shown)}: this engine says ` +
      `${got ? 'valid' : 'invalid'}, the ledger's verdicts assume ${want ? 'valid' : 'invalid'}. ` +
      `Running ${process.version}. Refusing to emit verdicts.`,
    );
    process.exit(3);
  }
}

const out = [];
for await (const line of createInterface({ input: process.stdin, crlfDelay: Infinity })) {
  const path = line.trim();
  if (!path) continue;
  let bytes;
  try {
    bytes = readFileSync(path);
  } catch (e) {
    // Not a verdict. See the header.
    console.error(`json oracle: cannot read ${path}: ${e.message}`);
    process.exit(2);
  }
  out.push(`${path}\t${isValid(bytes) ? 'valid' : 'invalid'}`);
}
process.stdout.write(out.length ? `${out.join('\n')}\n` : '');
