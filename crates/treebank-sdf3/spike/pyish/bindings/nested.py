def outer(a):
    def inner(b):
        return a + b
    return inner
add1 = outer(1)
