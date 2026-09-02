// What the playground puts in the editor when you pick a grammar.
//
// A module of its own, and not a const in playground.mjs, for one reason:
// playground.mjs bootstraps itself against `document` at import time, so
// anything that wants to READ this list -- `tests/playground.test.ts`, which
// checks that every grammar has one -- would have to fake a browser to get at
// it. Data that a test needs is data that belongs beside the view rather than
// inside it.
//
// The dropdown these fill is DERIVED from the crates: a directory with a
// grammar.js in it is a grammar. This list is not, which is the whole reason
// the test exists -- a language that arrives without a sample is in the menu,
// loads its parser, and shows an empty editor. Both `yaml` and `hcl` shipped
// exactly that way, on production, and nothing said so.
//
// Each one is meant to be READ, not just parsed: a few lines that show what
// the language looks like when it is being itself, and that light up the
// constructs the grammar reference is worth clicking into.

// biome-ignore-all lint/suspicious/noTemplateCurlyInString: `${var.x}` in the
// HCL sample is HCL's own interpolation, in a plain string that is never a
// template literal. The rule is looking for the mistake of writing `${}` in
// quotes and meaning a template; here the braces are the sample.
export const SAMPLES = {
  bash: 'greet() {\n  local name=${1:?need a name}\n  printf \'hi %s\\n\' "${name@Q}"\n}\n\nfor f in *.txt; do greet "$f"; done\n',
  c: "int main(void) {\n    int xs[] = {1, 2, 3};\n    return xs[0];\n}\n",
  hcl: 'terraform {\n  required_version = ">= 1.5"\n}\n\nvariable "environment" {\n  type    = string\n  default = "staging"\n}\n\nresource "aws_s3_bucket" "artifacts" {\n  bucket = "artifacts-${var.environment}"\n\n  tags = {\n    Environment = var.environment\n    ManagedBy   = "terraform"\n  }\n}\n\nlocals {\n  bucket_arns = [for b in aws_s3_bucket.artifacts : b.arn if b.arn != ""]\n\n  policy = <<-JSON\n    { "Version": "2012-10-17", "Statement": [] }\n  JSON\n}\n',
  cpp: "template <typename T>\nauto sum(const std::vector<T>& xs) -> T {\n    return std::accumulate(xs.begin(), xs.end(), T{});\n}\n",
  java: "record Point(int x, int y) {\n    Point {\n        if (x < 0) throw new IllegalArgumentException();\n    }\n}\n",
  python:
    "def greet(name: str = 'world') -> str:\n    match name.split():\n        case [first, *rest]:\n            return f'hello {first}'\n        case _:\n            return 'hello'\n",
  ruby: 'class Greeter\n  def initialize(name) = @name = name\n  def call = "hello #{@name}"\nend\n',
  rust: "fn largest<T: PartialOrd>(xs: &[T]) -> Option<&T> {\n    xs.iter().reduce(|a, b| if a > b { a } else { b })\n}\n",
  typescript:
    "type Result<T> = { ok: true; value: T } | { ok: false; error: string };\n\nconst unwrap = <T,>(r: Result<T>): T => {\n  if (!r.ok) throw new Error(r.error);\n  return r.value;\n};\n",
  yaml: "defaults: &defaults\n  retries: 3\n  timeout: 30s\n\nservices:\n  api:\n    <<: *defaults\n    image: registry.example.com/api:1.4.0\n    ports: [8080, 8443]\n    command: >-\n      serve --config /etc/api.yaml\n  worker:\n    <<: *defaults\n    script: |\n      set -eu\n      exec ./worker --queue jobs\n",
  zig: 'const std = @import("std");\n\npub fn main() !void {\n    const xs = [_]u8{ 1, 2, 3 };\n    std.debug.print("{d}\\n", .{xs.len});\n}\n',
};
