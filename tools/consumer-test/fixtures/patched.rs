// Every construct here is valid Rust that upstream tree-sitter-rust rejects,
// one per grammar-fix patch in crates/treebank-rust/ledger.json. If this file
// parses without an error node, the crate we resolved from the registry really
// does carry the treebank patch series.
extern "C" {
    pub type Foo;                                   // 0002 extern types
}

unsafe extern "C" {
    safe fn get_random_u64() -> u64;                // 0010 safe fn
}

macro_rules! tilde  { (~ $x:tt) => { $x }; }        // 0003 ~ in token trees
macro_rules! dollar { ($mode:ident, $) => { 1 }; }  // 0008 bare $

type Hours = ri8<-25, 25>;                          // 0006 negative const generics

struct _Test where Error: Send + Sync;              // 0012 unit struct + where

fn unit_where() -> u8 where (): Target {            // 0009 unit type in where
    let _x = str!["hi"];                            // 0005 primitive-named macro
    let _v = try!(g());                             // 0007 try! (2015 edition)
    let _s = "\u{4_e}";                             // 0013 underscores in escapes
    let _c = '\u{4_e}';
    0
}

fn attrs(t: S) {
    match t {
        S { #[cfg(x)] inner: _a, .. } => {}          // 0011 attrs on pattern fields
    }
}
