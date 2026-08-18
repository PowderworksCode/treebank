// A block-like expression ENDS the statement it is in. A following `*`, `-`
// or `&` starts a new statement; it is not a binary operator applied to the
// block.
fn f(c: bool, rem: &mut u8, buf: &mut Option<u8>, slice: u8, x: i32) {
    if c {
        *rem = 0;
    }
    *buf = Some(slice);

    match c {
        _ => (),
    }
    -x;

    loop {
        break;
    }
    &x;

    // ...and a macro statement still owns its semicolon.
    println!("x");
    // ...while a block-like macro still needs none.
    cfg_if::cfg_if! { if #[cfg(unix)] { fn g() {} } }
}

// In EXPRESSION position nothing changes: this is one multiplication.
fn h(c: bool) -> i32 {
    let y = if c { 1 } else { 2 } * 3;
    y
}
