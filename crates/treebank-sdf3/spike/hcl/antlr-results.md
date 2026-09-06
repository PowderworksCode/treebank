# ANTLR results for Hcl

3 of 17 corpus expectations hold under the ANTLR lowering.

## FAIL: literals, and the three keywords that are literals only where a value may go

expected:

```
(config_file (attribute name: (identifier) value: (integer)) (attribute name: (identifier) value: (float)) (attribute name: (identifier) value: (float)) (attribute name: (identifier) value: (true)) (attribute name: (identifier) value: (false)) (attribute name: (identifier) value: (null)) (attribute name: (identifier) value: (quoted_template (quote) (template_literal) (quote))))
```

got:

```
8:13 no viable alternative at input 'true       = "an attribute may be named for a keyword"'
```

## PASS: the operator ladder, with HCL's own precedence

## FAIL: the conditional, right associative, and not an ExprTerm

expected:

```
(config_file (attribute name: (identifier) value: (conditional condition: (binary_expression left: (get_attr operand: (identifier) name: (identifier)) right: (quoted_template (quote) (quote))) consequence: (get_attr operand: (identifier) name: (identifier)) alternative: (get_attr operand: (identifier) name: (identifier)))) (attribute name: (identifier) value: (conditional condition: (identifier) consequence: (identifier) alternative: (conditional condition: (identifier) consequence: (identifier) alternative: (identifier)))))
```

got:

```
2:23 no viable alternative at input '\nname = var.override != ""'
```

## FAIL: tuples and objects, with the separators each admits

expected:

```
(config_file (attribute name: (identifier) value: (tuple)) (attribute name: (identifier) value: (tuple (quoted_template (quote) (template_literal) (quote)) (quoted_template (quote) (template_literal) (quote)) (quoted_template (quote) (template_literal) (quote)))) (attribute name: (identifier) value: (tuple (integer) (integer))) (attribute name: (identifier) value: (object)) (attribute name: (identifier) value: (object (object_elem key: (identifier) value: (integer)) (object_elem key: (identifier) value: (integer)))) (attribute name: (identifier) value: (object (object_elem key: (quoted_template (quote) (template_literal) (quote)) value: (integer)))) (attribute name: (identifier) value: (object (object_elem key: (parenthesized_expression (get_attr operand: (identifier) name: (identifier))) value: (integer)))) (attribute name: (identifier) value: (object (object_elem key: (identifier) value: (quoted_template (quote) (template_literal) (quote))) (object_elem key: (identifier) value: (quoted_template (quote) (template_literal) (quote))))))
```

got:

```
3:16 no viable alternative at input 'zones        = ["a"'
```

## FAIL: for expressions over both collection kinds, with a condition and grouping

expected:

```
(config_file (attribute name: (identifier) value: (for_tuple_expr binding: (identifier) collection: (get_attr operand: (identifier) name: (identifier)) result: (get_attr operand: (identifier) name: (identifier)))) (attribute name: (identifier) value: (for_tuple_expr binding: (identifier) binding: (identifier) collection: (get_attr operand: (identifier) name: (identifier)) result: (quoted_template (quote) (template_interpolation expression: (identifier)) (template_literal) (template_interpolation expression: (get_attr operand: (identifier) name: (identifier))) (quote)))) (attribute name: (identifier) value: (for_tuple_expr binding: (identifier) collection: (get_attr operand: (identifier) name: (identifier)) result: (get_attr operand: (identifier) name: (identifier)) condition: (for_cond condition: (get_attr operand: (identifier) name: (identifier))))) (attribute name: (identifier) value: (for_object_expr binding: (identifier) collection: (get_attr operand: (identifier) name: (identifier)) key: (get_attr operand: (identifier) name: (identifier)) value: (get_attr operand: (identifier) name: (identifier)))) (attribute name: (identifier) value: (for_object_expr binding: (identifier) collection: (get_attr operand: (identifier) name: (identifier)) key: (get_attr operand: (identifier) name: (identifier)) value: (get_attr operand: (identifier) name: (identifier)) grouping: (ellipsis))) (attribute name: (identifier) value: (index operand: (for_tuple_expr binding: (identifier) binding: (identifier) collection: (get_attr operand: (identifier) name: (identifier)) result: (identifier) condition: (for_cond condition: (binary_expression left: (get_attr operand: (identifier) name: (identifier)) right: (quoted_template (quote) (quote))))) key: (integer))))
```

got:

```
3:38 no viable alternative at input 'indexed  = [for i, s in var.subnets : "${i}-${s.name}"'
```

## PASS: function calls, including a provider-namespaced one and an expanded argument

## FAIL: the access operators: attribute, index, legacy index, and both splats

expected:

```
(config_file (attribute name: (identifier) value: (get_attr operand: (get_attr operand: (identifier) name: (identifier)) name: (identifier))) (attribute name: (identifier) value: (index operand: (get_attr operand: (identifier) name: (identifier)) key: (integer))) (attribute name: (identifier) value: (legacy_index operand: (get_attr operand: (identifier) name: (identifier)))) (attribute name: (identifier) value: (attr_splat operand: (get_attr operand: (identifier) name: (identifier)) name: (identifier))) (attribute name: (identifier) value: (full_splat operand: (get_attr operand: (identifier) name: (identifier)) name: (identifier))) (attribute name: (identifier) value: (index operand: (get_attr operand: (index operand: (get_attr operand: (get_attr operand: (identifier) name: (identifier)) name: (identifier)) key: (integer)) name: (identifier)) key: (quoted_template (quote) (template_literal) (quote)))) (attribute name: (identifier) value: (get_attr operand: (function_call function: (function_name (identifier)) (arguments (get_attr operand: (identifier) name: (identifier)) (integer))) name: (identifier))))
```

got:

```
7:41 no viable alternative at input 'chained     = module.vpc.subnets[0].tags["Name"'
```

## FAIL: a resource block, its labels and a nested block

expected:

```
(config_file (block type: (identifier) body: (body (attribute name: (identifier) value: (quoted_template (quote) (template_literal) (quote))) (block type: (identifier) body: (body (attribute name: (identifier) value: (object (object_elem key: (identifier) value: (quoted_template (quote) (template_literal) (quote))) (object_elem key: (identifier) value: (quoted_template (quote) (template_literal) (quote))))))))) (block type: (identifier) label: (string_lit) label: (string_lit) body: (body (attribute name: (identifier) value: (quoted_template (quote) (template_literal) (quote))) (block type: (identifier) body: (body (attribute name: (identifier) value: (true)))))))
```

got:

```
3:21 no viable alternative at input '\nterraform {\n  required_version = ">= 1.5"'
```

## FAIL: a one-line block holds at most one attribute, and a multi-line body may be empty

expected:

```
(config_file (block type: (identifier) body: (body)) (block type: (identifier) label: (string_lit) body: (body (attribute name: (identifier) value: (quoted_template (quote) (template_literal) (quote))))) (block type: (identifier) body: (body)))
```

got:

```
4:30 no viable alternative at input 'variable "region" { default = "us-east-1"'
```

## PASS: naked identifier labels, and a dash in a name

## FAIL: comments in all three spellings, and blank lines between items

expected:

```
(config_file (comment) (comment) (block_comment) (attribute name: (identifier) value: (integer)) (attribute name: (identifier) value: (quoted_template (quote) (template_literal) (quote))) (comment))
```

got:

```
9:16 no viable alternative at input 'instance_type = "t3.micro"'
```

## FAIL: a .tfvars body: attributes only, no blocks

expected:

```
(config_file (attribute name: (identifier) value: (quoted_template (quote) (template_literal) (quote))) (attribute name: (identifier) value: (object (object_elem key: (identifier) value: (quoted_template (quote) (template_literal) (quote))) (object_elem key: (identifier) value: (quoted_template (quote) (template_literal) (quote))))))
```

got:

```
2:9 no viable alternative at input '\nregion = "us-west-2"'
```

## FAIL: a quoted template: literal runs, interpolations, escapes and the escaped introductions

expected:

```
(config_file (attribute name: (identifier) value: (quoted_template (quote) (template_literal) (quote))) (attribute name: (identifier) value: (quoted_template (quote) (template_literal) (template_interpolation expression: (get_attr operand: (identifier) name: (identifier))) (template_literal) (quote))) (attribute name: (identifier) value: (quoted_template (quote) (template_interpolation expression: (get_attr operand: (identifier) name: (identifier))) (template_interpolation expression: (get_attr operand: (identifier) name: (identifier))) (quote))) (attribute name: (identifier) value: (quoted_template (quote) (template_literal (escape_sequence) (escape_sequence) (escape_sequence)) (quote))) (attribute name: (identifier) value: (quoted_template (quote) (template_literal) (quote))) (attribute name: (identifier) value: (quoted_template (quote) (template_interpolation expression: (get_attr operand: (identifier) name: (identifier))) (quote))) (attribute name: (identifier) value: (quoted_template (quote) (quote))))
```

got:

```
2:13 no viable alternative at input '\nplain      = "no interpolation here"'
```

## FAIL: a heredoc, in both markers, and its terminator rules

expected:

```
(config_file (attribute name: (identifier) value: (heredoc_template (heredoc_start) (template_literal) (heredoc_end))) (attribute name: (identifier) value: (heredoc_template (heredoc_start) (template_literal) (template_interpolation expression: (get_attr operand: (identifier) name: (identifier))) (template_literal) (heredoc_end))) (attribute name: (identifier) value: (heredoc_template (heredoc_start) (heredoc_end))))
```

got:

```
2:9 no viable alternative at input '\npolicy = <'
```

## FAIL: template directives, nested, with an else branch and both strip markers

expected:

```
(config_file (attribute name: (identifier) value: (quoted_template (quote) (template_if condition: (binary_expression left: (get_attr operand: (identifier) name: (identifier)) right: (quoted_template (quote) (quote))) consequence: (template_body (template_literal) (template_interpolation expression: (get_attr operand: (identifier) name: (identifier)))) (else_clause alternative: (template_body (template_literal)))) (quote))) (attribute name: (identifier) value: (quoted_template (quote) (template_for binding: (identifier) collection: (get_attr operand: (identifier) name: (identifier)) body: (template_body (template_interpolation expression: (get_attr operand: (identifier) name: (identifier))) (template_literal))) (quote))) (attribute name: (identifier) value: (quoted_template (quote) (template_if condition: (get_attr operand: (identifier) name: (identifier)) consequence: (template_body (template_literal))) (quote))) (attribute name: (identifier) value: (quoted_template (quote) (template_for binding: (identifier) binding: (identifier) collection: (get_attr operand: (identifier) name: (identifier)) body: (template_body (template_interpolation expression: (identifier)) (template_literal) (template_interpolation expression: (identifier)) (template_literal))) (quote))))
```

got:

```
2:11 no viable alternative at input '\ngreeting = "%{ if var.name != "'
```

## FAIL: a heredoc carries the same template sub-language, and either may nest in the other

expected:

```
(config_file (attribute name: (identifier) value: (heredoc_template (heredoc_start) (template_for binding: (identifier) collection: (get_attr operand: (identifier) name: (identifier)) body: (template_body (template_literal) (template_interpolation expression: (get_attr operand: (identifier) name: (identifier))) (template_literal))) (template_literal) (heredoc_end))) (attribute name: (identifier) value: (quoted_template (quote) (template_interpolation expression: (heredoc_template (heredoc_start) (template_literal) (heredoc_end))) (quote))))
```

got:

```
8:9 token recognition error at: '"${ <<EOT\n'; 11:1 token recognition error at: '"\n'; 2:9 no viable alternative at input '\nscript = <'
```

## FAIL: block labels are string literals, not templates: no interpolation reaches them

expected:

```
(config_file (block type: (identifier) label: (string_lit) label: (string_lit) body: (body (attribute name: (identifier) value: (quoted_template (quote) (template_literal) (quote))))))
```

got:

```
3:8 no viable alternative at input '\nresource "aws_instance" "web-01" {\n  ami = "ami-123"'
```

