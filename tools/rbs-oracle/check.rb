# Syntax-only RBS validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# The reference parser is RBS's own — `RBS::Parser.parse_signature`, the call
# `rbs` itself makes to turn a signature file's text into declarations. It
# parses and stops: no type checking, no constant resolution, and it never
# loads the signatures a file references, so a signature naming a class it
# cannot see is not an error and each file is judged entirely on its own
# text. Same property that makes `ast.parse` usable for Python and
# `ts.createSourceFile` for TypeScript.
#
# RBS IS NOT RUBY, which is the whole reason this grammar exists separately.
# `def foo(a) = a + 1` is Ruby and this parser rejects it; `def foo: (Integer)
# -> String` is RBS and CRuby rejects it. The two corpora come out of the same
# gems and must never be pointed at each other's oracle.
#
# THE VERSION IS LOAD-BEARING, more so here than for any other oracle in this
# repo. Measured on the top-1000 gem corpus of 2,216 .rbs files:
#
#   rbs 2.8.2 (bundled with CRuby 3.2)   48 invalid
#   rbs 4.1.3                             1 invalid
#
# Forty-seven of those forty-eight were the toolchain, not the files — and
# forty-six of them were the `rbs` gem's OWN core signatures, written in a
# syntax its own older parser predates. An RBS oracle pinned to whatever
# happens to ship with the interpreter would report a corpus 2% broken and
# file every one of those as noise. Hence the explicit `gem` activation
# below: a version too old to judge this corpus must fail loudly at startup
# rather than quietly produce verdicts about itself.
gem "rbs", ">= 4.0"
require "rbs"
require "pathname"

def parses?(path, src)
  RBS::Parser.parse_signature(RBS::Buffer.new(name: Pathname(path), content: src))
  true
rescue RBS::ParsingError, RBS::Parser::SyntaxError
  false
rescue NoMethodError, ArgumentError, TypeError, RangeError
  # A few malformed inputs surface as plain Ruby errors out of the C parser
  # rather than as a ParsingError. They still mean "this file is not RBS".
  false
end

$stdin.each_line do |line|
  path = line.strip
  next if path.empty?
  begin
    src = File.read(path, encoding: "UTF-8")
  rescue SystemCallError, IOError => e
    # A file we cannot read is NOT an invalid file. `validate()` is called
    # only on files the grammar already failed, so every false "invalid"
    # erases a candidate grammar gap, silently. Fail loudly instead; the
    # sweep driver checks our exit status.
    $stderr.puts("rbs-oracle: cannot read #{path}: #{e.class}: #{e.message}")
    $stderr.puts("rbs-oracle: this is an oracle failure, not a verdict; check the corpus root")
    exit 1
  end
  $stdout.write("#{path}\t#{parses?(path, src) ? 'valid' : 'invalid'}\n")
end
$stdout.flush
