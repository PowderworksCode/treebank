//! Terms and the printer, without a parser in the loop: the CLI's parse
//! output for a small pyish program is imploded to the module's term and
//! printed through the templates, and the text is black's.

use treebank_sdf3::term::{parse_sexp, Imploder, Term};

const SOURCE: &str = "x = 1\ndef f(a):\n    return a + x  # trailing\nprint(f(2))\n";

// What `tree-sitter parse` prints for SOURCE with the pyish parser.
const SEXP: &str = "(program [0, 0] - [4, 0]
  (assign [0, 0] - [1, 0]
    target: (id [0, 0] - [0, 1])
    value: (exp_int [0, 4] - [0, 5]
      (int [0, 4] - [0, 5])))
  (def [1, 0] - [3, 0]
    name: (id [1, 4] - [1, 5])
    parameters: (param [1, 6] - [1, 7]
      name: (id [1, 6] - [1, 7]))
    body: (block [2, 4] - [3, 0]
      (return [2, 4] - [3, 0]
        value: (add [2, 11] - [2, 16]
          left: (id [2, 11] - [2, 12])
          right: (id [2, 15] - [2, 16]))
        (comment [2, 18] - [2, 28]))))
  (print [3, 0] - [4, 0]
    value: (call [3, 6] - [3, 10]
      function: (id [3, 6] - [3, 7])
      arguments: (exp_int [3, 8] - [3, 9]
        (int [3, 8] - [3, 9])))))";

fn pyish() -> (treebank_sdf3::ast::Module, treebank_sdf3::lower::Names) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/spike/pyish/pyish.sdf3");
    let module = treebank_sdf3::load_module(std::path::Path::new(path)).unwrap();
    let names = treebank_sdf3::lower_all(&module).unwrap().lowered.names;
    (module, names)
}

#[test]
fn the_tree_implodes_to_the_signature_term() {
    let (module, names) = pyish();
    let cst = parse_sexp(SEXP).unwrap();
    let term = Imploder::new(&module, &names).implode(&cst, SOURCE).unwrap();
    assert_eq!(
        term.aterm(),
        r#"Program([Assign("x", Int("1")), Def("f", [Param("a")], Block([Return(Add("a", "x"))])), Print(Call("f", [Int("2")]))])"#
    );
    // The trailing comment survived as an annotation on the return.
    let Term::App { args, .. } = &term else { panic!() };
    let Term::List(stmts) = &args[0] else { panic!() };
    let Term::App { args: def_args, .. } = &stmts[1] else { panic!() };
    let Term::App { args: block_args, .. } = &def_args[2] else { panic!() };
    let Term::List(body) = &block_args[0] else { panic!() };
    let Term::App { trailing, .. } = &body[0] else { panic!() };
    assert_eq!(trailing.as_deref(), Some("# trailing"));
}

#[test]
fn the_templates_print_the_term_in_blacks_style() {
    let (module, names) = pyish();
    let cst = parse_sexp(SEXP).unwrap();
    let term = Imploder::new(&module, &names).implode(&cst, SOURCE).unwrap();
    let out = treebank_sdf3::print::Printer::new(&module).print(&term).unwrap();
    // Two blank lines around the def (`separate(2)`), a four-space body
    // (the template's indentation), two spaces before the comment.
    assert_eq!(
        out,
        "x = 1\n\n\ndef f(a):\n    return a + x  # trailing\n\n\nprint(f(2))\n"
    );
}

#[test]
fn a_productive_production_prints_like_a_template() {
    // `Exp.Int = INT` has no template; its one symbol is its layout.
    let (module, _) = pyish();
    let t = Term::App {
        sort: "Exp".into(),
        cons: "Int".into(),
        args: vec![Term::Str("42".into())],
        leading: vec![],
        trailing: None,
        blank_before: false,
    };
    assert_eq!(treebank_sdf3::print::Printer::new(&module).print(&t).unwrap(), "42\n");
}
