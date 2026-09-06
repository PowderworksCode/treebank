x = 1
def g():
    global x
    x = 2
    return x
print(g())
print(x)
def h(a):
    a = a + 1
    return a + x
print(h(1))
