// Neither a jump nor a range is callable, so a parenthesised expression
// after one is its value, not an argument list.
fn slice_range(payload: &[u8], from: usize, pos: usize) -> &[u8] {
    &payload[(from + pos)..(from + pos + 35)]
}

fn break_with_value(items: &[u8]) -> (Option<u8>, Option<u8>) {
    loop {
        match items.first() {
            None => break (None, None),
            Some(_) => continue,
        }
    }
}

// ...but an immediately-invoked closure with an explicit return type is a
// call whose callee is the closure.
fn immediate() -> u8 {
    let t = || -> u8 { 1 }();
    t
}
