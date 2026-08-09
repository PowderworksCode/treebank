// Patch 0004 is a strictness fix: rustc lexes `'a` as one token, so `' a` is
// not valid Rust, but upstream accepts it. This file MUST fail to parse. If it
// parses cleanly we resolved upstream's grammar, not ours.
fn f<' a>() {}
