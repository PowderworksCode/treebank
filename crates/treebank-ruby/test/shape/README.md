# Shape fixtures

Every file here is a mis-parse that was found on the corpus by
`treebank shape` — our tree parsed it cleanly and grouped it differently
from CRuby's — and then fixed. CI runs the shape gate over this
directory (it has no corpus), so each fixture pins its fix.

- `defined-paren.rb` — `defined?(x) && y` swallowed the conjunction into
  the operand; the paren form is a parse.y PRIMARY and binds like a call.
