# Format results for pyish

5 of 5 programs round-trip, print idempotently, and print exactly what black prints.

## PASS: pyish/programs/global_print.py

```py
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
```

## PASS: pyish/programs/whole_scope.py

```py
x = 1


def f():
    print(x)
    x = 2


f()
```

## PASS: rosetta/branching/program.py

```py
def classify(n):
    if n < 0:
        return 1
    else:
        return 2
```

## PASS: rosetta/comments/program.py

```py
# a leading comment
def greet(name):
    prefix = name  # a trailing comment
    return prefix + name
```

## PASS: rosetta/hello-roles/program.py

```py
def fetch(url, timeout):
    while url < timeout:
        return timeout
    return url


def method(this):
    return fetch(this, 3)
```

