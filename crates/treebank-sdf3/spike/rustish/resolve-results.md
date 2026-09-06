# Resolution results for rustish

5 of 5 programs print, under resolution from bindings.json alone, what rustc prints.

## PASS: block_expr.rs

```rs
fn main() {
    let x = 1;
    let y = {
        let x = 5;
        x + 1
    };
    println!("{}", y + x);
    let x = { x + y };
    println!("{}", x);
}
```

| | output |
|---|---|
| bindings.json | `7 7` |
| rustc | `7 7` |

## PASS: initializer.rs

```rs
fn main() {
    let a = 2;
    let b = a * 3;
    let a = b - a;
    let b = {
        let a = a + 1;
        a * b
    };
    println!("{}", a);
    println!("{}", b);
}
```

| | output |
|---|---|
| bindings.json | `4 30` |
| rustc | `4 30` |

## PASS: items.rs

```rs
fn main() {
    println!("{}", twice(4));
    fn twice(n: i64) -> i64 {
        n * 2
    }
    println!("{}", later(1));
}
fn later(n: i64) -> i64 {
    n + 100
}
```

| | output |
|---|---|
| bindings.json | `8 101` |
| rustc | `8 101` |

## PASS: params.rs

```rs
fn f(a: i64) -> i64 {
    let a = a + 1;
    let a = a + 1;
    a
}
fn main() {
    println!("{}", f(1));
}
```

| | output |
|---|---|
| bindings.json | `3` |
| rustc | `3` |

## PASS: shadow.rs

```rs
fn main() {
    let x = 1;
    println!("{}", x);
    let x = x + 10;
    println!("{}", x);
    {
        let x = x * 2;
        println!("{}", x);
    }
    println!("{}", x);
}
```

| | output |
|---|---|
| bindings.json | `1 11 22 11` |
| rustc | `1 11 22 11` |

