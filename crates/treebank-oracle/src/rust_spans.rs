//! Node BOUNDARIES from `syn`, for the shape check.
//!
//! Rust has no equivalent of `ts.createSourceFile` or CPython's `ast` sitting
//! in the toolchain, so the question was where to get a reference tree at
//! all. Three candidates, and the choice matters:
//!
//! - **HIR** is the obvious-sounding one and the wrong one. It is
//!   post-desugaring: `for` and `while` become `loop` plus `match`, `?`
//!   becomes a match on `Try`, closures are rewritten. Comparing our surface
//!   tree against it would report a disagreement at every one of those, none
//!   of which is a parser defect. HIR answers "what does this mean", and this
//!   check asks "how is this written".
//! - **rustc's AST** is the right level but not reachable: `-Zast-json` was
//!   removed in 2020, and `-Zunpretty=ast-tree` prints spans as session-global
//!   `BytePos` that would have to be mapped back per file, on nightly.
//! - **`syn`** is the right level and already a dependency, because the rust
//!   validity oracle uses it. `proc-macro2`'s `span-locations` feature turns
//!   every span into a file-relative `byte_range()`, which is exactly the
//!   currency this check trades in.
//!
//! So: `syn`, in-process, no subprocess at all.
//!
//! What `syn` does NOT see is worth stating. It is a parser for the token
//! stream, so ordinary `//` comments never existed by the time we look;
//! doc comments survive as attributes and are inside the item's span, the
//! same way our `_attribute` members are. And `syn` accepts only valid Rust,
//! so anything it rejects comes back `skipped` rather than counted as
//! agreement.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;
use syn::spanned::Spanned;
use syn::visit::Visit;

use crate::spans::{FileSpans, Span, SpanOracle};

pub struct RustSpans;

impl SpanOracle for RustSpans {
    fn spans(&self, srcroot: &Path, paths: &[String]) -> Result<HashMap<String, FileSpans>> {
        Ok(paths
            .par_iter()
            .map(|rel| {
                let full = srcroot.join(rel);
                let file = match std::fs::read_to_string(&full) {
                    Ok(src) => match syn::parse_file(&src) {
                        Ok(ast) => {
                            let mut v = SpanVisitor { spans: Vec::new() };
                            v.visit_file(&ast);
                            // `has_edges: false`: syn has no generic field
                            // reflection, so there is no honest way to name
                            // a child's role. Saying so beats inventing one.
                            FileSpans {
                                spans: v.spans,
                                edges: Vec::new(),
                                has_edges: false,
                                tokens: Vec::new(),
                                has_tokens: false,
                                error: None,
                                skipped: None,
                            }
                        }
                        // `syn` parses only valid Rust. A rejection is the
                        // sweep's business, not this check's; comparing
                        // shapes against a tree that does not exist is noise.
                        Err(e) => FileSpans {
                            spans: Vec::new(),
                            edges: Vec::new(),
                            has_edges: false,
                            tokens: Vec::new(),
                            has_tokens: false,
                            error: Some(e.span().byte_range().start),
                            skipped: Some(format!("syn: {e}")),
                        },
                    },
                    // Unlike a VERDICT oracle, an unreadable file here cannot
                    // flatter the grammar -- a skipped file is compared
                    // against nothing. Recording the reason is enough.
                    Err(e) => FileSpans {
                        spans: Vec::new(),
                        edges: Vec::new(),
                        has_edges: false,
                        tokens: Vec::new(),
                        has_tokens: false,
                        error: None,
                        skipped: Some(format!("read: {e}")),
                    },
                };
                (rel.clone(), file)
            })
            .collect())
    }
}

struct SpanVisitor {
    spans: Vec<Span>,
}

impl SpanVisitor {
    fn push<T: Spanned>(&mut self, node: &T, kind: &str) {
        let r = node.span().byte_range();
        if r.end > r.start {
            self.spans.push(Span { start: r.start, end: r.end, kind: kind.to_string() });
        }
    }
}

/// `syn`'s enums carry no name for their own variants, so the interesting
/// ones are spelled out. Coarse names would lump unrelated constructs into
/// one cluster and make the report useless for finding anything.
fn item_kind(i: &syn::Item) -> &'static str {
    use syn::Item::*;
    match i {
        Const(_) => "Item::Const", Enum(_) => "Item::Enum", ExternCrate(_) => "Item::ExternCrate",
        Fn(_) => "Item::Fn", ForeignMod(_) => "Item::ForeignMod", Impl(_) => "Item::Impl",
        Macro(_) => "Item::Macro", Mod(_) => "Item::Mod", Static(_) => "Item::Static",
        Struct(_) => "Item::Struct", Trait(_) => "Item::Trait", TraitAlias(_) => "Item::TraitAlias",
        Type(_) => "Item::Type", Union(_) => "Item::Union", Use(_) => "Item::Use",
        _ => "Item::Other",
    }
}

fn expr_kind(e: &syn::Expr) -> &'static str {
    use syn::Expr::*;
    match e {
        Array(_) => "Expr::Array", Assign(_) => "Expr::Assign", Async(_) => "Expr::Async",
        Await(_) => "Expr::Await", Binary(_) => "Expr::Binary", Block(_) => "Expr::Block",
        Break(_) => "Expr::Break", Call(_) => "Expr::Call", Cast(_) => "Expr::Cast",
        Closure(_) => "Expr::Closure", Const(_) => "Expr::Const", Continue(_) => "Expr::Continue",
        Field(_) => "Expr::Field", ForLoop(_) => "Expr::ForLoop", Group(_) => "Expr::Group",
        If(_) => "Expr::If", Index(_) => "Expr::Index", Infer(_) => "Expr::Infer",
        Let(_) => "Expr::Let", Lit(_) => "Expr::Lit", Loop(_) => "Expr::Loop",
        Macro(_) => "Expr::Macro", Match(_) => "Expr::Match", MethodCall(_) => "Expr::MethodCall",
        Paren(_) => "Expr::Paren", Path(_) => "Expr::Path", Range(_) => "Expr::Range",
        Reference(_) => "Expr::Reference", Repeat(_) => "Expr::Repeat", Return(_) => "Expr::Return",
        Struct(_) => "Expr::Struct", Try(_) => "Expr::Try", TryBlock(_) => "Expr::TryBlock",
        Tuple(_) => "Expr::Tuple", Unary(_) => "Expr::Unary", Unsafe(_) => "Expr::Unsafe",
        While(_) => "Expr::While", Yield(_) => "Expr::Yield",
        _ => "Expr::Other",
    }
}

fn pat_kind(p: &syn::Pat) -> &'static str {
    use syn::Pat::*;
    match p {
        Const(_) => "Pat::Const", Ident(_) => "Pat::Ident", Lit(_) => "Pat::Lit",
        Macro(_) => "Pat::Macro", Or(_) => "Pat::Or", Paren(_) => "Pat::Paren",
        Path(_) => "Pat::Path", Range(_) => "Pat::Range", Reference(_) => "Pat::Reference",
        Rest(_) => "Pat::Rest", Slice(_) => "Pat::Slice", Struct(_) => "Pat::Struct",
        Tuple(_) => "Pat::Tuple", TupleStruct(_) => "Pat::TupleStruct", Type(_) => "Pat::Type",
        Wild(_) => "Pat::Wild",
        _ => "Pat::Other",
    }
}

fn type_kind(t: &syn::Type) -> &'static str {
    use syn::Type::*;
    match t {
        Array(_) => "Type::Array", BareFn(_) => "Type::BareFn", Group(_) => "Type::Group",
        ImplTrait(_) => "Type::ImplTrait", Infer(_) => "Type::Infer", Macro(_) => "Type::Macro",
        Never(_) => "Type::Never", Paren(_) => "Type::Paren", Path(_) => "Type::Path",
        Ptr(_) => "Type::Ptr", Reference(_) => "Type::Reference", Slice(_) => "Type::Slice",
        TraitObject(_) => "Type::TraitObject", Tuple(_) => "Type::Tuple",
        _ => "Type::Other",
    }
}

impl<'ast> Visit<'ast> for SpanVisitor {
    fn visit_item(&mut self, i: &'ast syn::Item) {
        self.push(i, item_kind(i));
        syn::visit::visit_item(self, i);
    }
    fn visit_expr(&mut self, e: &'ast syn::Expr) {
        self.push(e, expr_kind(e));
        syn::visit::visit_expr(self, e);
    }
    fn visit_pat(&mut self, p: &'ast syn::Pat) {
        self.push(p, pat_kind(p));
        syn::visit::visit_pat(self, p);
    }
    fn visit_type(&mut self, t: &'ast syn::Type) {
        self.push(t, type_kind(t));
        syn::visit::visit_type(self, t);
    }
    fn visit_stmt(&mut self, s: &'ast syn::Stmt) {
        let kind = match s {
            syn::Stmt::Local(_) => "Stmt::Local",
            syn::Stmt::Item(_) => "Stmt::Item",
            syn::Stmt::Expr(..) => "Stmt::Expr",
            syn::Stmt::Macro(_) => "Stmt::Macro",
        };
        self.push(s, kind);
        syn::visit::visit_stmt(self, s);
    }
    fn visit_block(&mut self, b: &'ast syn::Block) {
        self.push(b, "Block");
        syn::visit::visit_block(self, b);
    }
    fn visit_arm(&mut self, a: &'ast syn::Arm) {
        self.push(a, "Arm");
        syn::visit::visit_arm(self, a);
    }
    fn visit_field(&mut self, f: &'ast syn::Field) {
        self.push(f, "Field");
        syn::visit::visit_field(self, f);
    }
    fn visit_variant(&mut self, v: &'ast syn::Variant) {
        self.push(v, "Variant");
        syn::visit::visit_variant(self, v);
    }
    fn visit_fn_arg(&mut self, a: &'ast syn::FnArg) {
        self.push(a, "FnArg");
        syn::visit::visit_fn_arg(self, a);
    }
    fn visit_impl_item(&mut self, i: &'ast syn::ImplItem) {
        self.push(i, "ImplItem");
        syn::visit::visit_impl_item(self, i);
    }
    fn visit_trait_item(&mut self, i: &'ast syn::TraitItem) {
        self.push(i, "TraitItem");
        syn::visit::visit_trait_item(self, i);
    }
    fn visit_attribute(&mut self, a: &'ast syn::Attribute) {
        self.push(a, "Attribute");
        syn::visit::visit_attribute(self, a);
    }
    fn visit_generic_param(&mut self, g: &'ast syn::GenericParam) {
        self.push(g, "GenericParam");
        syn::visit::visit_generic_param(self, g);
    }
    fn visit_where_predicate(&mut self, w: &'ast syn::WherePredicate) {
        self.push(w, "WherePredicate");
        syn::visit::visit_where_predicate(self, w);
    }
    fn visit_lifetime(&mut self, l: &'ast syn::Lifetime) {
        self.push(l, "Lifetime");
        syn::visit::visit_lifetime(self, l);
    }
}
