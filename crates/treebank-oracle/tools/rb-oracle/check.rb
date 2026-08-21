# Syntax-only Ruby validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# The reference parser is CRuby's own, driven through
# `RubyVM::AbstractSyntaxTree.parse_file`, which runs the same parser
# `ruby` runs and stops at the AST. It never requires, never executes and
# never resolves a constant, so a missing gem is not an error and a file is
# judged entirely on its own text — the same property that makes
# `compile(..., "exec")` usable for Python and `JavacTask.parse()` for Java.
# Verified rather than assumed: a file whose body is
# `File.write("/tmp/x", "boom"); raise "ran"` parses valid and writes
# nothing, `require 'no_such_gem'` is valid, and neither `BEGIN { }` nor
# `at_exit { }` fires.
#
# Why parse_file and not parse(File.read(path)). parse_file hands the path
# to the parser, so a PEP-263-equivalent `# -*- coding: -*-` / `# encoding:`
# magic comment is honoured by the parser itself, and the reported errors
# carry the real file name. Reading the bytes here and guessing the encoding
# would be us reimplementing a rule CRuby already owns.
#
# `error_tolerant:` is deliberately NOT passed. It exists to make the parser
# return a tree for broken input, which is precisely the verdict this oracle
# is here to give.
#
# The language version is whatever CRuby runs this, and for Ruby that is a
# real knob rather than an incidental: `it` as an implicit block parameter
# is 3.4+, hash shorthand `{x:}` and anonymous block forwarding `def a(&) =
# b(&)` are 3.1+, `=>` rightward assignment and endless methods are 3.0+.
# A file that needs syntax newer than this interpreter is not valid Ruby
# *here*, and recording it as corpus noise is the honest answer — but it is
# an answer about our toolchain, so ledger.json records the interpreter
# under `oracle` and the sweep manifest records it next to the verdict.
#
# Deliberately NOT tolerant in the other direction either: Ruby 1.8-only
# syntax (`{1 => 2}` is fine, but `when x: y` and `:"sym" =>` hash rocket
# variants that 1.8 allowed) is a syntax error to every supported CRuby,
# and calling it valid would turn the grammar's correct rejection into a
# reported grammar gap.

# Parser warnings ("ambiguous first argument", "assigned but unused
# variable") are not errors and would otherwise pour into the sweep's
# stderr. $VERBOSE = nil silences the verbose class; the Warning hook
# catches the rest, including those the parser emits unconditionally.
$VERBOSE = nil

module SilentWarnings
  def warn(*) = nil
end
Warning.extend(SilentWarnings)

def parses?(path)
  RubyVM::AbstractSyntaxTree.parse_file(path)
  true
rescue SyntaxError, EncodingError, ArgumentError
  # SyntaxError is the verdict. EncodingError/ArgumentError cover a source
  # whose bytes are not decodable under its declared encoding and embedded
  # NULs — `ruby` refuses those files too, so "invalid" is the same answer
  # it would give.
  false
rescue SystemStackError, NoMemoryError
  # Pathological nesting, which is a real thing in generated corpus files.
  false
end

# A file we cannot read is NOT invalid, and saying so would be the exact
# lie this oracle exists to prevent: `validate()` is called only on files
# the grammar already failed, so every "invalid" here erases a candidate
# grammar gap. A harness that hands us a bad path must fail loudly and be
# fixed. Found the hard way: an early run against relative paths from the
# wrong directory returned 604 of 604 "invalid" and looked like a clean
# result. The driver checks our exit status, so a nonzero exit is seen.
def unreadable!(path, err)
  $stderr.puts("rb-oracle: cannot read #{path}: #{err.class}: #{err.message}")
  exit 2
end

$stdin.each_line do |line|
  path = line.strip
  next if path.empty?
  begin
    verdict = parses?(path) ? 'valid' : 'invalid'
  rescue SystemCallError, IOError => e
    unreadable!(path, e)
  end
  $stdout.write("#{path}\t#{verdict}\n")
end
$stdout.flush
