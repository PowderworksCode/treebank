# The conditional is the LOOSEST operator: `a or b if c else d` is
# `(a or b) if c else d`, not `a or (b if c else d)`.
a = 1
b = 2
c = True
x = a or b if c else None
y = a and b if c else None
# ...and a parenthesised conditional is still an operand.
z = a or (b if c else None)
