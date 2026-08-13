// YAML validity check for the treebank oracle.
//
// stdin:  one absolute file path per line
// stdout: "<path>\tvalid|invalid" per line
//
// THE ORACLE IS A POSITION, NOT A FACT. YAML has no reference implementation
// to appeal to: libyaml, go-yaml, snakeyaml and js-yaml disagree with each
// other on real documents, and measured over the official conformance suite
// they disagree with it too. So this file records a choice, and `ledger.json`'s
// `oracle` block records the evidence for it. Everything below is the reason
// the choice is what it is.
//
// WHAT: js-yaml's `parseEvents`, the parse stage — the event stream, before
// composition, tag resolution or construction. Measured against
// yaml/yaml-test-suite data-2022-01-17 (402 cases, 94 expected errors):
//
//   libyaml 0.2.5 parse (PyYAML CParser)  83.3%   16 accepts-invalid  51 rejects-valid
//   go-yaml v3.0.1 -> yaml.Node           81.8%   15                  58
//   eemeli `yaml` 2.9.0 CST parser        76.6%   94                   0   <- no rejection power
//   eemeli `yaml` 2.9.0 parseAllDocuments 99.3%    2                   1
//   js-yaml 5.2.3 loadAll                 92.0%    0                  32
//   js-yaml 5.2.3 parseEvents (this)     100.0%    0                   0
//
// `rejects-valid` is the column that decides it. `Lang::validate` only ever
// runs on files the grammar already failed, so a valid file called invalid is
// recorded as corpus NOISE and hides a grammar gap. libyaml — which ROADMAP
// names for this language — does that to 51 of the suite's 308 valid cases.
//
// WHY NOT the later stages, when a stricter oracle sounds safer: measured on
// 3217 real corpus files, parse and load disagree on 73 (2.27%) while libyaml
// and go-yaml at the same stage disagree on 0. The 73 are unresolvable tags
// (ansible's `!vault`) and duplicate keys — neither is a syntax property, and
// a tree-sitter grammar cannot see either, so rejecting them would book real
// gaps as noise. For this language the STAGE is a bigger lever than the
// PARSER, which is the opposite of what one expects.
//
// THE DECODE LAYER IS PART OF THE ORACLE. js-yaml takes a JS string, so
// something must turn bytes into characters, and that something implements a
// piece of YAML 1.2.2 section 5.2 whether or not anyone writes it down. The
// naive `readFileSync(path, 'utf8')` silently does three wrong things —
// substitutes U+FFFD for ill-formed UTF-8, mojibakes UTF-16, and leaves a
// leading BOM in the string for the parser to count as content. Measured on a
// 21-case encoding/control battery that cost four wrong verdicts; libyaml
// scores 0 there precisely because it does this layer in C. Doing it
// explicitly here takes js-yaml from 4 wrong to 1 while holding 402/402 on
// the suite. Choosing js-yaml means owning this function.
//
// The one case still wrong is a mid-stream BOM (`a: 1\n<BOM>b: 2\n`), which
// libyaml and go-yaml reject and js-yaml and eemeli accept. It is left as is
// because the spec does not clearly settle it: section 5.2 permits a BOM at
// stream start and before subsequent documents, while U+FEFF is itself inside
// `c-printable` (xE000-xFFFD). A 2-2 split on a rule nobody can quote is the
// whole reason this oracle is declared as a position.
import { createInterface } from "node:readline";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { parseEvents } from "js-yaml";

// The pinned version is a DIALECT, exactly as it is for lua's interpreter:
// what "invalid YAML" means here is what js-yaml 5.2.x's event parser says,
// and adjudicating with whatever happens to be installed would silently
// produce verdicts for an unrecorded oracle. `ledger.json`'s `oracle.version`
// names the same version this refuses to run without.
const WANT_MAJOR = 5;
const installed = createRequire(import.meta.url)("js-yaml/package.json").version;
if (Number(installed.split(".")[0]) !== WANT_MAJOR) {
  process.stderr.write(
    `oracle: js-yaml ${installed} is installed but this oracle is written for ` +
      `${WANT_MAJOR}.x; refusing to emit verdicts for an unrecorded dialect ` +
      `(npm ci in tools/yaml-oracle)\n`,
  );
  process.exit(1);
}

/**
 * Bytes -> characters, per YAML 1.2.2 section 5.2.
 *
 * Throwing here is a VERDICT of invalid, not an I/O failure: the encoding is
 * part of the character stream's well-formedness, so ill-formed bytes are
 * not YAML. That is why the caller keeps this in a different try block from
 * the read.
 */
function decode(b) {
  // UTF-32 first: its BOMs start with the UTF-16 BOMs' bytes, so testing
  // UTF-16 first would misidentify every UTF-32 stream.
  if (b.length >= 4 && b[0] === 0 && b[1] === 0 && b[2] === 0xfe && b[3] === 0xff) {
    return utf32(b.subarray(4), true);
  }
  if (b.length >= 4 && b[0] === 0xff && b[1] === 0xfe && b[2] === 0 && b[3] === 0) {
    return utf32(b.subarray(4), false);
  }
  if (b.length >= 2 && b[0] === 0xfe && b[1] === 0xff) {
    return new TextDecoder("utf-16be", { fatal: true }).decode(b.subarray(2));
  }
  if (b.length >= 2 && b[0] === 0xff && b[1] === 0xfe) {
    return new TextDecoder("utf-16le", { fatal: true }).decode(b.subarray(2));
  }
  // UTF-8. `fatal` is the load-bearing option: without it ill-formed bytes
  // become U+FFFD and the file parses as valid YAML containing replacement
  // characters, which is the gap-MANUFACTURING direction — the grammar
  // cannot parse those bytes either, so a fix agent would be dispatched at a
  // file that no parser accepts.
  const text = new TextDecoder("utf-8", { fatal: true }).decode(b);
  // Exactly one leading BOM, and only a leading one. Stripping is safe here
  // and is not universally safe: the parallel tbtoml session found that
  // pre-stripping would break `toml`, which distinguishes leading (valid),
  // doubled (invalid) and mid-stream (invalid) BOMs. Measured for YAML:
  // libyaml, go-yaml and js-yaml all accept a DOUBLED BOM, because U+FEFF is
  // inside `c-printable`, so this strip changes no verdict it should not.
  return text.charCodeAt(0) === 0xfeff ? text.slice(1) : text;
}

function utf32(b, be) {
  if (b.length % 4 !== 0) throw new TypeError("truncated UTF-32 stream");
  let s = "";
  for (let i = 0; i < b.length; i += 4) {
    const cp = be
      ? (b[i] << 24) | (b[i + 1] << 16) | (b[i + 2] << 8) | b[i + 3]
      : (b[i + 3] << 24) | (b[i + 2] << 16) | (b[i + 1] << 8) | b[i];
    if (cp < 0 || cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff)) {
      throw new TypeError("invalid UTF-32 code point");
    }
    s += String.fromCodePoint(cp);
  }
  return s;
}

const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
const out = [];
for await (const path of rl) {
  if (!path) continue;
  let bytes;
  try {
    bytes = readFileSync(path);
  } catch (e) {
    // AN UNREADABLE FILE IS NOT AN INVALID FILE. `validate()` runs only on
    // files the grammar already failed and an `invalid` verdict books the
    // file as noise, so a mistyped corpus root that produced verdicts would
    // drive gap_files to zero and report a flawless grammar. A directory
    // lands here too (EISDIR), which is deliberate: go-yaml's `os.Open`
    // succeeds on a directory and its driver books one as `invalid`, and
    // taplo exits 0 on a path matching nothing, which is the same defect in
    // the more expensive direction.
    process.stderr.write(`oracle: cannot read ${path}: ${e.message}\n`);
    process.exit(1);
  }
  let valid = false;
  try {
    for (const _event of parseEvents(decode(bytes))) {
      // Drained, not collected: the verdict is whether the stream of events
      // can be produced at all.
    }
    valid = true;
  } catch {
    valid = false;
  }
  out.push(`${path}\t${valid ? "valid" : "invalid"}`);
}
process.stdout.write(out.length ? `${out.join("\n")}\n` : "");
