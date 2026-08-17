def outer():
    if x:
        body()

    # This comment belongs to what FOLLOWS it, so the if statement above
    # must not extend past it.
    if y:
        other()

    try:
        risky()
        # A comment before a continuation clause stays inside the body it
        # follows -- the statement is not over.
    except ValueError:
        return  # ...and a same-line trailing comment is trivia.
