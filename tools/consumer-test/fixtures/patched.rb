# Every construct here needs a treebank patch; upstream tree-sitter-ruby
# rejects this file. One line per patch, so a regression names itself.
class Fixture
  # 0003 Ruby 3.1 endless method with a command call body
  def compile(**options) = raise NotImplementedError, 'subclass responsibility'

  # 0004 block argument with a space, and anonymous forwarding on its own line
  def self.execute(args, & block)
    new(args).execute(& block)
  end

  # 0005 self and super as method names
  def super(times = 1)
  end

  # 0008 defined? as a hash key
  NODES = {defined?: 1, def: 2, if: 3}

  # 0012 element reference on a string with a space before the bracket
  FLAG = 'attributes' ['a']
end

# 0006 symbols for punctuation globals and non-ASCII names
PATHS = {:$: => [:$LOAD_PATH], :☠ => :exit}

# 0007 character literal with an escaped control character
GROUP_SEPARATOR = ?\C-\]

# 0009 an embedded document ends only at a line starting with =end
=begin
  parse_subtree(["=begin\n", "<<< name\n", "=end\n"])
=end

# 0010 call with a line break before the .() arguments
RESULT = Fixture.
  (1, 2)

# 0011 more than one call chained onto a do block
should.raise Foo do
  bar
end.message.should.match(/x/)
