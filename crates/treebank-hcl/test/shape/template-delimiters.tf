# The `"` of a quoted template and the `<<EOT` / `EOT` of a heredoc were
# HIDDEN external tokens, so three lexemes the language spells had no node
# anywhere in the tree and hclsyntax's own tokens for them had no boundary
# to match. They are `quote`, `heredoc_start` and `heredoc_end` now.
name = "plain"

# The closing delimiter line's trailing spaces are part of it, which is what
# hclsyntax's `TokenCHeredoc` spans; ending the node at the delimiter word
# left them in nobody's node.
description = <<-DESCRIPTION
  a heredoc whose terminator line has trailing whitespace
  DESCRIPTION  

# A quoted template nested inside an interpolation inside a heredoc. The
# scanner's mode stack has to push and pop in step with the parse; when both
# quote symbols were reachable through one rule it popped where it should
# have pushed, and 32 corpus files stopped parsing.
command = <<EOT
  aws ecs run-task --network-configuration "subnets=[${join(",", var.subnets)}]"
EOT
