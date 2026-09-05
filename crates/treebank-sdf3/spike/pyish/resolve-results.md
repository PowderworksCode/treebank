# Resolution results for pyish

2 of 2 programs print, under resolution from bindings.json alone, what python3 prints.

## PASS: global_print.py

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

| | output |
|---|---|
| bindings.json | `2 2 4` |
| python3 | `2 2 4` |

## PASS: whole_scope.py

```py
x = 1
def f():
    print(x)
    x = 2
f()
```

| | output |
|---|---|
| bindings.json | `error: x used before its binding` |
| python3 | `error: UnboundLocalError: cannot access local variable 'x' where it is not associated with a value` |

