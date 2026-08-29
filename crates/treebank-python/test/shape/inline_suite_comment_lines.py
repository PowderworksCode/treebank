# An inline suite opens no indent level, so the comment lines between it and
# its continuation clause have nowhere to be absorbed. Two or more of them
# used to end the statement early and leave `else` with no `if`.
def render(instance, force):
    if instance: #and force: #XXX: colons in a comment are not a suite
        if '(' in repr(force): lines.append(force)
       #else: #XXX: comments may sit at their OWN indent, under the code
       #    reconstructor = force.__reduce__()
       #    _ = reconstructor()
        else: # a trailing comment on the clause line is trivia
            lines = dumpsource(force)

    # The same shape through the other clause keywords.
    for i in force: pass
    #1
    #2
    else: pass

    try: pass
    #1
    #2
    except ValueError: pass
    #3
    #4
    finally: pass
