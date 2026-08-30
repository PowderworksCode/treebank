// The shared body of both YAML oracle legs.
//
// stdin:  one file path per line
// stdout: "<path>\tvalid|invalid" per line
//
// `yaml` (eemeli) is the reference-grade YAML 1.2 processor: measured on
// yaml-test-suite it answers 402 of the 406 cases correctly, against 373
// for js-yaml, 353 for ruamel and 335 for PyYAML. It is also the only
// implementation available that can be ASKED the version question — the
// same parser under `version: "1.1"` — which is what makes YAML's version
// union something this repository can MEASURE rather than assume.
//
// `parseAllDocuments` runs the parser and the composer and reports both
// through `doc.errors`. Warnings are deliberately NOT read: a duplicate key
// and an unknown tag are warnings there, both are resolution rather than
// syntax, and a grammar has no opinion about either.
import fs from "node:fs";
import YAML from "yaml";

// An unreadable file is NOT an invalid file. Returning "invalid" for one
// looks harmless and is not: validate() is only ever called on files the
// grammar already failed, and an invalid verdict records the file as corpus
// NOISE. So a mistyped corpus root would make every path unreadable, every
// grammar failure noise, gap_files zero -- and the sweep would report a
// flawless grammar. A broken oracle must fail loudly, never quietly agree.
function read(path) {
  try {
    return fs.readFileSync(path, "utf8");
  } catch (e) {
    process.stderr.write(`yaml-oracle: cannot read ${path}: ${e.message}\n`);
    process.exit(1);
  }
}

export function judge(version) {
  const valid = (source) => {
    try {
      const docs = YAML.parseAllDocuments(source, {
        version,
        prettyErrors: false,
      });
      for (const doc of docs) {
        if (doc.errors.length > 0) return false;
      }
      return true;
    } catch {
      // A throw out of the parser is still a rejection of the text.
      return false;
    }
  };

  const answer = (path) => {
    process.stdout.write(`${path}\t${valid(read(path)) ? "valid" : "invalid"}\n`);
  };

  let buffer = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => {
    buffer += chunk;
    let nl;
    while ((nl = buffer.indexOf("\n")) >= 0) {
      const path = buffer.slice(0, nl);
      buffer = buffer.slice(nl + 1);
      if (path.length > 0) answer(path);
    }
  });
  process.stdin.on("end", () => {
    if (buffer.length > 0) answer(buffer);
  });
}
