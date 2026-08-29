# A backslash reached at a NON-ZERO column fixes the indent there: this
# suite opens at 4, not at the 8 the first token sits at, so the `x = 1`
# back at 4 stays inside the block.
if True:
    \
        1
    x = 1
    y = 2

# A backslash at column ZERO fixes nothing; the indent is the continuation
# line's own, so `pass` is the class body.
class Plotter:
\
    pass

# ...and one space before it is already non-zero, so this body sits at 1.
class C:
 \
pass

# The same rule through a loop's else, and through a mid-line continuation,
# where the column of the backslash means nothing at all.
for i in r:
    \
        i
    i
else:\
    done
