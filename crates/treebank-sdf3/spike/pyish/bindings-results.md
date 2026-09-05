# Bindings results for pyish

6 of 6 programs classify every name in every scope as CPython's symtable does.

## PASS: basic.py

```python
x = 1
def f(a, b):
    y = a + b
    return y + x
z = f(1, 2)
```

| scope | name | bindings.json | symtable |
|---|---|---|---|
| top | `f` | local | local |
| top | `x` | local | local |
| top | `z` | local | local |
| f | `a` | parameter | parameter |
| f | `b` | parameter | parameter |
| f | `x` | global | global |
| f | `y` | local | local |

## PASS: control.py

```python
n = 3
def count():
    i = 0
    while i < n:
        i = i + 1
    else_taken = 0
    if i:
        pass
    else:
        else_taken = 1
    return i + else_taken
```

| scope | name | bindings.json | symtable |
|---|---|---|---|
| top | `count` | local | local |
| top | `n` | local | local |
| count | `else_taken` | local | local |
| count | `i` | local | local |
| count | `n` | global | global |

## PASS: forward.py

```python
def f():
    if 1 < 2:
        r = t
    t = 3
    return r
```

| scope | name | bindings.json | symtable |
|---|---|---|---|
| top | `f` | local | local |
| f | `r` | local | local |
| f | `t` | local | local |

## PASS: global_stmt.py

```python
x = 1
def g():
    global x
    x = 2
    return x
g()
```

| scope | name | bindings.json | symtable |
|---|---|---|---|
| top | `g` | local | local |
| top | `x` | local | local |
| g | `x` | global | global |

## PASS: nested.py

```python
def outer(a):
    def inner(b):
        return a + b
    return inner
add1 = outer(1)
```

| scope | name | bindings.json | symtable |
|---|---|---|---|
| top | `add1` | local | local |
| top | `outer` | local | local |
| inner | `a` | free | free |
| inner | `b` | parameter | parameter |
| outer | `a` | parameter | parameter |
| outer | `inner` | local | local |

## PASS: shadow.py

```python
x = 1
y = 2
def f(x):
    y = x + 1
    return y
def g():
    return x + y
```

| scope | name | bindings.json | symtable |
|---|---|---|---|
| top | `f` | local | local |
| top | `g` | local | local |
| top | `x` | local | local |
| top | `y` | local | local |
| f | `x` | parameter | parameter |
| f | `y` | local | local |
| g | `x` | global | global |
| g | `y` | global | global |

