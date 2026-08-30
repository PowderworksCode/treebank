# A `template_literal` is one node per RUN, not per chunk. It was one per
# chunk, and hclsyntax coalesces its pieces into a single `LiteralValueExpr`
# — so every escape sequence and every heredoc line boundary was a node
# boundary the reference parser did not have. 117 of the first 2,000 corpus
# files disagreed.
escaped = "a\tb\"c\\d and a tab\there"

# The same run, split by an interpolation on one side and an escape on the
# other: the literals before and after are separate runs, and the pieces
# within each are not.
mixed = "prefix\t${var.name}\tsuffix"

policy = <<EOT
{
  "Version": "2012-10-17",
  "Statement": [
    { "Effect": "Allow", "Resource": "${var.arn}" }
  ]
}
EOT
