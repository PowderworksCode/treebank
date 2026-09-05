# Format results for rustish

8 of 8 programs round-trip, print idempotently, and print exactly what rustfmt prints.

## PASS: rustish/programs/block_expr.rs

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

## PASS: rustish/programs/initializer.rs

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

## PASS: rustish/programs/items.rs

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

## PASS: rustish/programs/params.rs

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

## PASS: rustish/programs/shadow.rs

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

## PASS: rosetta/branching/program.rs

```rs
fn classify(n: i64) -> i64 {
    if n < 0 {
        return 1;
    } else {
        return 2;
    }
}
```

## PASS: rosetta/comments/program.rs

```rs
// a leading comment
fn greet(name: i64) -> i64 {
    let prefix = name; // a trailing comment
    return prefix + name;
}
```

## PASS: rosetta/hello-roles/program.rs

```rs
fn fetch(url: i64, timeout: i64) -> i64 {
    while url < timeout {
        return timeout;
    }
    return url;
}

fn method(this: i64) -> i64 {
    return fetch(this, 3);
}
```

