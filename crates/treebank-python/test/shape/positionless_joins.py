# CPython gives withitem and comprehension no source positions. The span
# oracle reconstructs them from their children and the real keyword token.
# A comment containing "for" must not be mistaken for that keyword.
[
    value
    for  # for
    item  # item
    in  # in
    source
]

# Parenthesized with-items and tuple-valued with-items use the same spelling.
# The first item's range excludes the grouping parentheses; the second one's
# range includes the tuple parentheses.
with (first, second):
    pass

with (item := 42,):
    pass

# Only the outermost pair below groups the with-item list. Inner pairs belong
# to the item expression and therefore to the reconstructed withitem range.
with (((nested))):
    pass

with ((assigned := 1)):
    pass

# A named expression and yield cannot be bare with-item expressions, so these
# single pairs belong to the expressions rather than a with-item list.
with (assigned := 2):
    pass

with (open("first")), (open("second")):
    pass

def generator(source):
    with (yield source):
        pass
