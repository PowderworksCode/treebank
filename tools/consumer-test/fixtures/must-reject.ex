# Invalid Elixir by syntax alone — Code.string_to_quoted/2 rejects it, and so
# must the grammar. The sigil is the near-miss for patch 0003: an ODD number
# of backslashes means the last one escapes the delimiter, so the sigil is
# genuinely unterminated and the patch must not "fix" this one too.
x = ~S(\\\)
