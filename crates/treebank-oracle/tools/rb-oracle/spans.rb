# Node BOUNDARIES from CRuby, for the shape check.
#
# stdin:  one file path per line
# stdout: one JSON object per line,
#         {"path":..., "spans":[[start,end,kind],...], "tokens":[[start,end],...]}
#
# Same claim as py-oracle/spans.py: not a comparison of node NAMES — that
# needs a correspondence table and rots — but of where the boundaries fall.
# If CRuby says something spans bytes 15..20 and our tree has no node with
# exactly that span, the two parsers disagree about the shape of the code,
# whatever either calls the node.
#
# Offsets are absolute BYTES. RubyVM::AbstractSyntaxTree reports
# (lineno, column) where the column is a byte offset within its line —
# verified against multibyte source, not assumed — so only the line starts
# have to be added back. Ripper's lexer columns are byte offsets too.
#
# No edges: a CRuby AST node's children are positional (`Node#children`
# with no names), the same situation syn is in for rust, so every record
# says `has_edges: false` rather than letting an empty list claim the file
# has no connections.

require 'json'
require 'ripper'

# Ripper token kinds that mark layout rather than text. Everything else has
# a real extent to compare against our lexer's.
LAYOUT = %i[
  on_sp on_nl on_ignored_nl on_ignored_sp on_words_sep on___end__
].freeze

def line_starts(data)
  starts = [0, 0]
  data.each_byte.with_index { |b, i| starts << i + 1 if b == 0x0A }
  starts
end

def spans_of(root, starts, size)
  out = []
  stack = [root]
  until stack.empty?
    node = stack.pop
    next unless node.is_a?(RubyVM::AbstractSyntaxTree::Node)
    node.children.each { |c| stack.push(c) }
    fl = node.first_lineno
    ll = node.last_lineno
    next if fl.nil? || fl <= 0 || ll >= starts.length
    s = starts[fl] + node.first_column
    e = starts[ll] + node.last_column
    out << [s, e, node.type.to_s] if s >= 0 && s < e && e <= size
  end
  out
end

def tokens_of(data, path)
  src = data.dup.force_encoding(Encoding::UTF_8)
  return nil unless src.valid_encoding?
  starts = line_starts(data)
  out = []
  toks = Ripper.lex(src, path)
  return nil if toks.nil?
  toks.each do |(pos, kind, tok, _state)|
    next if LAYOUT.include?(kind)
    line, col = pos
    next if line >= starts.length
    s = starts[line] + col
    bytes = tok.bytesize
    # Ripper hands a comment token WITH its terminating newline, and a
    # heredoc terminator with the newline that ends its line; ours end at
    # the last content byte. That is a fact about where CRuby cuts the
    # stream, not a boundary disagreement, so normalise it here the way the
    # python oracle normalises UTF-16 columns.
    if %i[on_comment on_heredoc_end].include?(kind) && tok.end_with?("\n")
      bytes -= 1
      bytes -= 1 if tok.end_with?("\r\n")
    end
    e = s + bytes
    out << [s, e] if s >= 0 && s < e && e <= data.bytesize
  end
  out
rescue StandardError, SyntaxError
  # The lexer gave up part way; what it produced is not a complete account
  # of the file, so report none of it.
  nil
end

def error_offset(path, starts)
  # parse_file's SyntaxError carries no position — only the offending line
  # and a caret. compile_file's does ("path:LINE: syntax error"), so ask it
  # a second time just for the number. The caret snippet is often
  # truncated, so the line start is the honest offset we can certify.
  begin
    RubyVM::InstructionSequence.compile_file(path)
  rescue SyntaxError => e
    m = /:(\d+): syntax error/.match(e.message.to_s)
    return nil unless m
    line = m[1].to_i
    return nil unless line > 0 && line < starts.length
    return starts[line]
  rescue StandardError
    nil
  end
  nil
end

$stdin.each_line do |line|
  path = line.strip
  next if path.empty?

  begin
    data = File.binread(path)
  rescue StandardError => e
    # An unreadable file is an oracle FAILURE, never a verdict.
    warn "rb-oracle: cannot read #{path}: #{e.message}"
    exit 1
  end

  record = { 'path' => path, 'spans' => [], 'has_edges' => false }

  # Columns are byte offsets into the source AS CRUBY DECODED IT. A magic
  # comment naming a non-UTF-8 encoding, or a BOM, makes that a different
  # byte string from the file our parser read, and every offset after the
  # first difference is meaningless. Say so instead of reporting offsets
  # that do not line up.
  head = data[0, 512].to_s
  enc = head[/^#.*?coding[:=]\s*([\w.-]+)/, 1] || head[/\A#!.*\n#.*?coding[:=]\s*([\w.-]+)/, 1]
  if data.start_with?("\xEF\xBB\xBF".b) || (enc && !%w[utf-8 utf8 us-ascii ascii].include?(enc.downcase))
    record['skipped'] = "source encoding #{enc || 'BOM'}: byte offsets would not line up"
    puts JSON.generate(record)
    next
  end

  starts = line_starts(data)
  begin
    root = RubyVM::AbstractSyntaxTree.parse_file(path)
  rescue SyntaxError => e
    # Only clean parses have meaningful boundaries; where it failed is
    # still worth reporting, because rejecting the right file at the wrong
    # offset misleads every gap investigation downstream.
    record['skipped'] = 'parse: SyntaxError'
    _ = e
    off = error_offset(path, starts)
    record['error'] = off if off
    puts JSON.generate(record)
    next
  rescue StandardError => e
    record['skipped'] = "parse: #{e.class}"
    puts JSON.generate(record)
    next
  end

  begin
    record['spans'] = spans_of(root, starts, data.bytesize)
    toks = tokens_of(data, path)
    record['tokens'] = toks unless toks.nil?
  rescue StandardError
    record['skipped'] = 'walk: error'
  end
  puts JSON.generate(record)
end
