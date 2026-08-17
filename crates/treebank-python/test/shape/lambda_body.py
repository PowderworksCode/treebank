A = {}
B = {}
v = 1
c = True
a = 1
b = True

# A lambda body is greedy: the conditional is the BODY, not something the
# lambda sits inside.
f = lambda x: A[x] if v == 1 else B[x]
g = lambda: 1 if c else 2
# ...but the else branch may be a bare lambda, and parentheses still work.
h = a if b else lambda: c
i = (lambda: 1) if c else 2

# Lambda parameters carry no annotations, so this is a dict whose KEY is a
# lambda -- not a set holding a lambda with an annotated parameter.
d = {lambda x: x: 1}
