//! Load a Treebank wasm pack and parse with it.
//!
//! A pack is one standalone module: the tree-sitter runtime, one grammar and
//! a small ABI, statically linked. It imports only WASI — six file-descriptor
//! calls the parse path never reaches — so hosting one needs a WASI runtime
//! and nothing else. No C toolchain, no `tree-sitter` crate, no per-grammar
//! dependency.
//!
//! That is the point of doing it this way. There are nine grammars and there
//! will be more, and a crate per grammar would mean a consumer picking
//! versions for each and a release for every one of them. Instead there is
//! this crate, and a `.wasm` fetched at runtime:
//!
//! ```no_run
//! use treebank::pack::Pack;
//!
//! let pack = Pack::from_path("treebank-python.wasm")?;
//! let tree = pack.parse("def f(x):\n    return x + 1\n")?;
//! println!("{}", tree.root().sexp()?);
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! The module answers for itself: [`Pack::provenance`] says which grammar and
//! what the last sweep measured, and [`Pack::terms`] returns the nominal
//! manifest that [`crate::expand`] needs, so a nominal query can be expanded
//! without shipping the grammar's `terms.json` alongside the parser.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use wasmer::{Function, Imports, Instance, Memory, Module, Store, TypedFunction, Value};

/// What a pack says about itself, read out of the module rather than from a
/// file beside it. A pack copied somewhere else still answers.
#[derive(Debug, Clone, Deserialize)]
pub struct Provenance {
    pub language: String,
    pub grammar_name: String,
    pub vocabulary: String,
    pub generate_cli: String,
    pub runtime: String,
    pub pack_abi: u32,
    #[serde(default)]
    pub versions: String,
    #[serde(default)]
    pub sources: BTreeMap<String, String>,
    /// Sweep shapes differ by language, because each oracle reports what it
    /// can honestly measure. Kept as raw JSON rather than forced into one
    /// struct that would be wrong for most of them.
    #[serde(default)]
    pub sweeps: serde_json::Value,
}

/// The nominal manifest a pack carries, which is what a nominal query is
/// expanded against.
#[derive(Debug, Clone, Deserialize)]
pub struct PackTerms {
    /// `facets` is accepted as an alias so a pack published before the
    /// rename still loads; see notes/vocabulary-naming.md §5.
    #[serde(default, alias = "facets")]
    pub nominal: BTreeMap<String, Vec<String>>,
}

const NAMED: u32 = 1;
const IS_ERROR: u32 = 2;
const HAS_ERROR: u32 = 4;
const MISSING: u32 = 8;

/// A loaded grammar.
pub struct Pack {
    store: std::cell::RefCell<Store>,
    memory: Memory,
    f: Abi,
    provenance: Provenance,
    terms: PackTerms,
    terms_raw: String,
    /// node-types.json as it ships inside the module. Held as bytes and
    /// parsed on demand: it is 40-70 KB, and parsing it on every load would
    /// cost more than the warm load itself, for something only a nominal
    /// query with a field constraint ever reads.
    node_types_raw: Option<String>,
    node_types: std::cell::OnceCell<Option<crate::node_types::NodeTypes>>,
}

struct Abi {
    strlen: TypedFunction<u32, u32>,
    alloc: TypedFunction<u32, u32>,
    free: TypedFunction<u32, ()>,
    parse: TypedFunction<(u32, u32), u32>,
    tree_free: TypedFunction<u32, ()>,
    tree_root: TypedFunction<(u32, u32), ()>,
    node_new: TypedFunction<(), u32>,
    node_free: TypedFunction<u32, ()>,
    node_type: TypedFunction<u32, u32>,
    node_sexp: TypedFunction<u32, u32>,
    node_flags: TypedFunction<u32, u32>,
    node_start_byte: TypedFunction<u32, u32>,
    node_end_byte: TypedFunction<u32, u32>,
    node_child_count: TypedFunction<u32, u32>,
    node_child: TypedFunction<(u32, u32, u32), u32>,
    node_named_child_count: TypedFunction<u32, u32>,
    node_named_child: TypedFunction<(u32, u32, u32), u32>,
    field_name_for_child: TypedFunction<(u32, u32), u32>,
    cstr_free: TypedFunction<u32, ()>,
    // Optional, because a pack is a versioned contract and a newer loader
    // must still drive an older pack. Queries arrived at pack_abi 3; every
    // pack published before that has none of these, and everything else in
    // this ABI works exactly the same without them.
    query: Option<QueryAbi>,
}

struct QueryAbi {
    new: TypedFunction<(u32, u32, u32, u32), u32>,
    delete: TypedFunction<u32, ()>,
    exec: TypedFunction<(u32, u32), u32>,
    cursor_delete: TypedFunction<u32, ()>,
    next_capture: TypedFunction<(u32, u32, u32, u32, u32), u32>,
    capture_name: TypedFunction<(u32, u32), u32>,
}

impl Pack {
    /// Load a pack from a file.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        Self::from_bytes(&bytes)
    }

    /// Load a pack from bytes, which is what a consumer that fetched one over
    /// HTTP has.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut store = Store::default();
        let module = compile_cached(&store, bytes)?;

        let imports = refuse_every_import(&module, &mut store)?;
        let instance =
            Instance::new(&mut store, &module, &imports).context("instantiating the pack")?;

        // Reactor exec model: _initialize runs the module's constructors.
        instance
            .exports
            .get_typed_function::<(), ()>(&store, "_initialize")
            .context("pack has no _initialize; is it a treebank pack?")?
            .call(&mut store)?;

        let memory = instance
            .exports
            .get_memory("memory")
            .map_err(|_| anyhow!("pack exports no memory"))?
            .clone();
        let f = Abi::bind(&instance, &mut store)?;

        let provenance = read_json(&mut store, &memory, &instance, "tb_provenance")?;
        // `tb_terms` since the vocabulary rename; `tb_roles` is the same
        // document under the old export name, and packs carrying it are
        // still on the CDN because packs are content-addressed and are
        // never rebuilt in place.
        let terms_raw = match read_string(&mut store, &memory, &instance, "tb_terms") {
            Ok(raw) => raw,
            Err(_) => read_string(&mut store, &memory, &instance, "tb_roles")?,
        };
        let terms = serde_json::from_str(&terms_raw).context("parsing tb_terms")?;
        // Optional: a pack built before this export exists still parses and
        // still expands nominal terms, just without the filtering.
        let node_types_raw = read_string(&mut store, &memory, &instance, "tb_node_types").ok();

        Ok(Self {
            store: std::cell::RefCell::new(store),
            memory,
            f,
            provenance,
            terms,
            terms_raw,
            node_types_raw,
            node_types: std::cell::OnceCell::new(),
        })
    }

    /// What the pack says about itself.
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// The nominal manifest, for [`crate::expand`].
    pub fn terms(&self) -> &PackTerms {
        &self.terms
    }

    /// The nominal manifest as it ships inside the module.
    ///
    /// [`Pack::terms`] is the same document parsed. This is for a consumer
    /// that has its own representation of the vocabulary and would otherwise
    /// have to convert out of ours and back — which is how two consumers come
    /// to disagree about what a `_callable` is.
    pub fn terms_json(&self) -> &str {
        &self.terms_raw
    }

    /// The node manifest as it ships inside the module, for the same reason.
    ///
    /// `None` when the pack predates the export. [`Pack::node_types`] is the
    /// parsed form this crate uses itself.
    pub fn node_types_json(&self) -> Option<&str> {
        self.node_types_raw.as_deref()
    }

    /// The node manifest this pack carries, parsed on first use.
    ///
    /// This is what tells `expand_query` that `lambda` has no `name` field.
    /// `None` when the pack predates the export or the manifest will not
    /// parse -- in which case expansion still happens, just unfiltered.
    pub fn node_types(&self) -> Option<&crate::node_types::NodeTypes> {
        self.node_types
            .get_or_init(|| {
                let raw = self.node_types_raw.as_deref()?;
                crate::node_types::NodeTypes::parse(raw).ok()
            })
            .as_ref()
    }

    /// The language this pack parses.
    pub fn language(&self) -> &str {
        &self.provenance.language
    }

    /// Expand a nominal query against this pack's manifest.
    ///
    /// `(_callable)` becomes `[(function_definition) (lambda)]`, which is what
    /// makes a query portable across grammars: the term is the same word
    /// everywhere and its members are whatever this grammar calls them.
    pub fn expand_query(&self, query: &str) -> Result<String> {
        crate::expand::expand_with_types(query, &self.terms.nominal, self.node_types())
    }

    /// Parse source text.
    pub fn parse(&self, source: &str) -> Result<Tree<'_>> {
        let bytes = source.as_bytes();
        let len = u32::try_from(bytes.len()).context("source larger than 4 GiB")?;
        let mut store = self.store.borrow_mut();

        let ptr = self.f.alloc.call(&mut *store, len)?;
        if ptr == 0 {
            return Err(anyhow!("the pack could not allocate {len} bytes"));
        }
        self.memory
            .view(&*store)
            .write(ptr as u64, bytes)
            .context("writing source into the pack")?;
        let tree = self.f.parse.call(&mut *store, ptr, len)?;
        self.f.free.call(&mut *store, ptr)?;
        if tree == 0 {
            return Err(anyhow!("parse failed"));
        }
        Ok(Tree {
            pack: self,
            handle: tree,
        })
    }
}

/// Every import a pack declares, answered with `badf`.
///
/// A pack imports six WASI file-descriptor calls because wasi-libc links
/// them, not because parsing reaches them. Supplying a real WASI context
/// would give a grammar the ability to open files; refusing every call
/// instead makes "a pack touches nothing" a property of the host rather
/// than a claim about the pack.
///
/// Anything outside `wasi_snapshot_preview1` is refused outright, because a
/// treebank pack has no business importing it.
fn refuse_every_import(module: &Module, store: &mut Store) -> Result<Imports> {
    let mut imports = Imports::new();
    for import in module.imports() {
        if import.module() != "wasi_snapshot_preview1" {
            bail!(
                "a pack may import only WASI, but this one imports {}::{}",
                import.module(),
                import.name()
            );
        }
        let signature = match import.ty() {
            wasmer::ExternType::Function(signature) => signature.clone(),
            other => bail!("a pack imports a non-function {other:?}"),
        };
        imports.define(
            import.module(),
            import.name(),
            Function::new(store, &signature, |_: &[Value]| {
                Ok(vec![Value::I32(WASI_BADF)])
            }),
        );
    }
    Ok(imports)
}

/// WASI `errno::badf` -- "bad file descriptor", the answer to every call.
const WASI_BADF: i32 = 8;

/// A parsed tree. Freed when dropped.
pub struct Tree<'p> {
    pack: &'p Pack,
    handle: u32,
}

impl<'p> Tree<'p> {
    pub fn root(&self) -> Node<'p> {
        let mut store = self.pack.store.borrow_mut();
        let node = self.pack.f.node_new.call(&mut *store).expect("tb_node_new");
        self.pack
            .f
            .tree_root
            .call(&mut *store, self.handle, node)
            .expect("tb_tree_root");
        Node {
            pack: self.pack,
            handle: node,
        }
    }
}

impl Drop for Tree<'_> {
    fn drop(&mut self) {
        let mut store = self.pack.store.borrow_mut();
        let _ = self.pack.f.tree_free.call(&mut *store, self.handle);
    }
}

/// A node in a parsed tree. Freed when dropped.
pub struct Node<'p> {
    pack: &'p Pack,
    handle: u32,
}

impl<'p> Node<'p> {
    /// The node's type, as a query would name it.
    pub fn kind(&self) -> Result<String> {
        let mut store = self.pack.store.borrow_mut();
        let ptr = self.pack.f.node_type.call(&mut *store, self.handle)?;
        read_cstr(&mut store, &self.pack.memory, &self.pack.f, ptr)
    }

    /// The whole subtree as an s-expression.
    pub fn sexp(&self) -> Result<String> {
        let mut store = self.pack.store.borrow_mut();
        let ptr = self.pack.f.node_sexp.call(&mut *store, self.handle)?;
        let out = read_cstr(&mut store, &self.pack.memory, &self.pack.f, ptr)?;
        self.pack.f.cstr_free.call(&mut *store, ptr)?;
        Ok(out)
    }

    /// Byte range in the source that was parsed.
    pub fn byte_range(&self) -> Result<std::ops::Range<usize>> {
        let mut store = self.pack.store.borrow_mut();
        let start = self.pack.f.node_start_byte.call(&mut *store, self.handle)?;
        let end = self.pack.f.node_end_byte.call(&mut *store, self.handle)?;
        Ok(start as usize..end as usize)
    }

    pub fn is_named(&self) -> Result<bool> {
        Ok(self.flags()? & NAMED != 0)
    }

    /// This node is an `ERROR` or a `MISSING`.
    pub fn is_error(&self) -> Result<bool> {
        Ok(self.flags()? & (IS_ERROR | MISSING) != 0)
    }

    /// This node, or something under it, is an error. Cheap: it is a flag on
    /// the node rather than a walk.
    pub fn has_error(&self) -> Result<bool> {
        Ok(self.flags()? & (HAS_ERROR | IS_ERROR | MISSING) != 0)
    }

    fn flags(&self) -> Result<u32> {
        let mut store = self.pack.store.borrow_mut();
        Ok(self.pack.f.node_flags.call(&mut *store, self.handle)?)
    }

    pub fn child_count(&self, named_only: bool) -> Result<u32> {
        let mut store = self.pack.store.borrow_mut();
        let f = if named_only {
            &self.pack.f.node_named_child_count
        } else {
            &self.pack.f.node_child_count
        };
        Ok(f.call(&mut *store, self.handle)?)
    }

    /// The `index`-th child, or `None` when the index is past the end. The ABI
    /// reports that rather than returning a null node, so it is surfaced
    /// rather than handed back as a node that would answer nonsense.
    pub fn child(&self, index: u32, named_only: bool) -> Result<Option<Node<'p>>> {
        let mut store = self.pack.store.borrow_mut();
        let kid = self.pack.f.node_new.call(&mut *store)?;
        let f = if named_only {
            &self.pack.f.node_named_child
        } else {
            &self.pack.f.node_child
        };
        let ok = f.call(&mut *store, self.handle, index, kid)?;
        if ok == 0 {
            self.pack.f.node_free.call(&mut *store, kid)?;
            return Ok(None);
        }
        Ok(Some(Node {
            pack: self.pack,
            handle: kid,
        }))
    }

    /// The field name the PARENT gives its `index`-th child, which is the edge
    /// label a query uses. Field names belong to the parent's view of a child,
    /// which is why this is asked here rather than of the child.
    pub fn field_name_for_child(&self, index: u32) -> Result<Option<String>> {
        let mut store = self.pack.store.borrow_mut();
        let ptr = self
            .pack
            .f
            .field_name_for_child
            .call(&mut *store, self.handle, index)?;
        if ptr == 0 {
            return Ok(None);
        }
        Ok(Some(read_cstr(
            &mut store,
            &self.pack.memory,
            &self.pack.f,
            ptr,
        )?))
    }

    /// Every named child, as a vector.
    pub fn named_children(&self) -> Result<Vec<Node<'p>>> {
        let count = self.child_count(true)?;
        (0..count)
            .filter_map(|i| self.child(i, true).transpose())
            .collect()
    }
}

impl Drop for Node<'_> {
    fn drop(&mut self) {
        let mut store = self.pack.store.borrow_mut();
        let _ = self.pack.f.node_free.call(&mut *store, self.handle);
    }
}

/// One capture from a query.
#[derive(Debug, Clone)]
pub struct Capture {
    /// The `@name` the pattern gave it, without the at sign.
    pub name: String,
    /// The node's type.
    pub kind: String,
    /// Byte range in the source that was parsed.
    pub range: std::ops::Range<usize>,
    /// Which pattern in the query matched, counting from zero.
    pub pattern: u32,
}

impl Pack {
    /// Run a query and collect its captures.
    ///
    /// The query is expanded against this pack's nominal manifest first, so a
    /// term written once runs against every grammar:
    ///
    /// ```no_run
    /// # use treebank::Pack;
    /// # let pack = Pack::fetch("python")?;
    /// # let tree = pack.parse("def f(): pass")?;
    /// for c in pack.query(&tree, "(_declaration) @decl")? {
    ///     println!("{} {:?}", c.kind, c.range);
    /// }
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// `(_declaration)` is structural and queryable directly; `(_callable)`
    /// is nominal and is rewritten into this grammar's members on the way in.
    /// Either way the caller writes the same query for every language.
    pub fn query(&self, tree: &Tree<'_>, query: &str) -> Result<Vec<Capture>> {
        let expanded = self.expand_query(query)?;
        let root = tree.root();
        self.query_node(&root, &expanded)
    }

    /// Run a query rooted at one node, taking the query exactly as given.
    /// [`Pack::query`] is this plus nominal expansion.
    pub fn query_node(&self, node: &Node<'_>, expanded: &str) -> Result<Vec<Capture>> {
        let q = self.f.query.as_ref().ok_or_else(|| {
            anyhow!(
                "this {} pack cannot run queries: it is pack_abi {}, and queries need 3. \
                 Fetch a current pack, or use expand_query and your own query engine.",
                self.provenance.language,
                self.provenance.pack_abi
            )
        })?;
        let bytes = expanded.as_bytes();
        let len = u32::try_from(bytes.len()).context("query larger than 4 GiB")?;
        let mut store = self.store.borrow_mut();

        // Two out-params for the error position, and the source itself, all
        // live in the module's memory.
        let src = self.f.alloc.call(&mut *store, len)?;
        let errs = self.f.alloc.call(&mut *store, 8)?;
        if src == 0 || errs == 0 {
            return Err(anyhow!("the pack could not allocate for a query"));
        }
        self.memory.view(&*store).write(src as u64, bytes)?;

        let handle = q.new.call(&mut *store, src, len, errs, errs + 4)?;
        if handle == 0 {
            // A query is usually written by a person, so where it broke is
            // the whole message.
            let mut raw = [0u8; 8];
            self.memory.view(&*store).read(errs as u64, &mut raw)?;
            let offset = u32::from_le_bytes(raw[0..4].try_into().unwrap()) as usize;
            let kind = u32::from_le_bytes(raw[4..8].try_into().unwrap());
            self.f.free.call(&mut *store, src)?;
            self.f.free.call(&mut *store, errs)?;
            return Err(anyhow!(
                "{} at byte {offset} of the query:\n  {expanded}\n  {:>offset$}^",
                query_error(kind),
                ""
            ));
        }
        self.f.free.call(&mut *store, src)?;
        self.f.free.call(&mut *store, errs)?;

        let cursor = q.exec.call(&mut *store, handle, node.handle)?;
        if cursor == 0 {
            q.delete.call(&mut *store, handle)?;
            return Err(anyhow!("the pack could not allocate a query cursor"));
        }

        let out_node = self.f.node_new.call(&mut *store)?;
        let out = self.f.alloc.call(&mut *store, 8)?;
        let mut found = Vec::new();
        let mut names: std::collections::HashMap<u32, String> = std::collections::HashMap::new();

        loop {
            let more = q
                .next_capture
                .call(&mut *store, cursor, handle, out_node, out, out + 4)?;
            if more == 0 {
                break;
            }
            let mut raw = [0u8; 8];
            self.memory.view(&*store).read(out as u64, &mut raw)?;
            let pattern = u32::from_le_bytes(raw[0..4].try_into().unwrap());
            let capture = u32::from_le_bytes(raw[4..8].try_into().unwrap());

            let kind_ptr = self.f.node_type.call(&mut *store, out_node)?;
            let kind = read_cstr(&mut store, &self.memory, &self.f, kind_ptr)?;
            let start = self.f.node_start_byte.call(&mut *store, out_node)? as usize;
            let end = self.f.node_end_byte.call(&mut *store, out_node)? as usize;

            // Capture names repeat across every match, and each lookup is a
            // call plus a string read, so they are resolved once.
            let name = match names.get(&capture) {
                Some(name) => name.clone(),
                None => {
                    let ptr = q.capture_name.call(&mut *store, handle, capture)?;
                    let name = read_cstr(&mut store, &self.memory, &self.f, ptr)?;
                    names.insert(capture, name.clone());
                    name
                }
            };

            found.push(Capture {
                name,
                kind,
                range: start..end,
                pattern,
            });
        }

        self.f.free.call(&mut *store, out)?;
        self.f.node_free.call(&mut *store, out_node)?;
        q.cursor_delete.call(&mut *store, cursor)?;
        q.delete.call(&mut *store, handle)?;
        Ok(found)
    }
}

/// TSQueryError, which the C header numbers rather than names. The numbering
/// is the header's and nothing else: an earlier version of this function
/// invented a `predicate` variant at 5, which pushed every later value along
/// and made a structure error -- the one a nominal query hits most -- report a
/// predicate the query did not contain.
fn query_error(kind: u32) -> &'static str {
    match kind {
        1 => "the query is not valid s-expression syntax",
        2 => "the query names a node type this grammar does not have",
        3 => "the query names a field this grammar does not have",
        4 => "the query captures something that cannot be captured",
        // TSQueryErrorStructure: the pattern's shape cannot occur. Usually a
        // field asked of a node type that does not declare it, which is what
        // an expanded nominal term produces when one member lacks the field.
        5 => {
            "the query asks for a shape this grammar cannot produce, \
              usually a field on a node type that does not have it"
        }
        6 => "the query names a language that is not this one",
        _ => "the query is not valid",
    }
}

#[cfg(test)]
mod query_error_tests {
    use super::query_error;

    /// Pinned against tree_sitter/api.h. The values are positional in the C
    /// enum, so inserting one shifts every later message onto the wrong error.
    #[test]
    fn the_numbering_matches_the_header() {
        assert!(query_error(1).contains("s-expression"));
        assert!(query_error(2).contains("node type"));
        assert!(query_error(3).contains("field"));
        assert!(query_error(4).contains("captures"));
        assert!(query_error(5).contains("shape"));
        assert!(query_error(6).contains("language"));
        assert_eq!(query_error(7), "the query is not valid");
        assert_eq!(query_error(99), "the query is not valid");
    }
}

fn read_cstr(store: &mut Store, memory: &Memory, f: &Abi, ptr: u32) -> Result<String> {
    if ptr == 0 {
        return Ok(String::new());
    }
    let len = f.strlen.call(&mut *store, ptr)? as usize;
    let mut buf = vec![0u8; len];
    memory.view(&*store).read(ptr as u64, &mut buf)?;
    Ok(String::from_utf8(buf)?)
}

fn read_json<T: for<'de> Deserialize<'de>>(
    store: &mut Store,
    memory: &Memory,
    instance: &Instance,
    name: &str,
) -> Result<T> {
    let ptr = instance
        .exports
        .get_typed_function::<(), u32>(&*store, name)
        .with_context(|| format!("pack has no {name}"))?
        .call(&mut *store)?;
    let len = instance
        .exports
        .get_typed_function::<(), u32>(&*store, &format!("{name}_len"))
        .with_context(|| format!("pack has no {name}_len"))?
        .call(&mut *store)? as usize;
    let mut buf = vec![0u8; len];
    memory.view(&*store).read(ptr as u64, &mut buf)?;
    serde_json::from_slice(&buf).with_context(|| format!("parsing {name}"))
}

/// The same shape as [`read_json`], stopping at the bytes. Used for a
/// manifest that is expensive to parse and not always needed.
fn read_string(
    store: &mut Store,
    memory: &Memory,
    instance: &Instance,
    name: &str,
) -> Result<String> {
    let ptr = instance
        .exports
        .get_typed_function::<(), u32>(&*store, name)
        .with_context(|| format!("pack has no {name}"))?
        .call(&mut *store)?;
    let len = instance
        .exports
        .get_typed_function::<(), u32>(&*store, &format!("{name}_len"))
        .with_context(|| format!("pack has no {name}_len"))?
        .call(&mut *store)? as usize;
    let mut buf = vec![0u8; len];
    memory.view(&*store).read(ptr as u64, &mut buf)?;
    String::from_utf8(buf).with_context(|| format!("{name} is not utf-8"))
}

impl QueryAbi {
    /// All six or none: a pack that exports some but not all of these is not
    /// something to half-drive.
    fn bind(instance: &Instance, store: &mut Store) -> Option<Self> {
        Some(Self {
            new: instance
                .exports
                .get_typed_function(&*store, "tb_query_new")
                .ok()?,
            delete: instance
                .exports
                .get_typed_function(&*store, "tb_query_delete")
                .ok()?,
            exec: instance
                .exports
                .get_typed_function(&*store, "tb_query_exec")
                .ok()?,
            cursor_delete: instance
                .exports
                .get_typed_function(&*store, "tb_query_cursor_delete")
                .ok()?,
            next_capture: instance
                .exports
                .get_typed_function(&*store, "tb_query_next_capture")
                .ok()?,
            capture_name: instance
                .exports
                .get_typed_function(&*store, "tb_query_capture_name")
                .ok()?,
        })
    }
}

impl Abi {
    fn bind(instance: &Instance, store: &mut Store) -> Result<Self> {
        macro_rules! f {
            ($name:literal) => {
                instance
                    .exports
                    .get_typed_function(&*store, $name)
                    .with_context(|| format!("pack has no {}", $name))?
            };
        }
        Ok(Self {
            strlen: f!("tb_strlen"),
            alloc: f!("tb_alloc"),
            free: f!("tb_free"),
            parse: f!("tb_parse"),
            tree_free: f!("tb_tree_free"),
            tree_root: f!("tb_tree_root"),
            node_new: f!("tb_node_new"),
            node_free: f!("tb_node_free"),
            node_type: f!("tb_node_type"),
            node_sexp: f!("tb_node_sexp"),
            node_flags: f!("tb_node_flags"),
            node_start_byte: f!("tb_node_start_byte"),
            node_end_byte: f!("tb_node_end_byte"),
            node_child_count: f!("tb_node_child_count"),
            node_child: f!("tb_node_child"),
            node_named_child_count: f!("tb_node_named_child_count"),
            node_named_child: f!("tb_node_named_child"),
            field_name_for_child: f!("tb_node_field_name_for_child"),
            cstr_free: f!("tb_cstr_free"),
            query: QueryAbi::bind(instance, store),
        })
    }
}

/// Compiling a pack costs a few hundred milliseconds; loading an
/// already-compiled one costs a few. Measured on a release build: python
/// 297ms cold against 1ms warm, C++ 362ms against 15ms. Everything else here
/// is far cheaper -- reading the file and parsing a small program are both
/// under a millisecond -- so this is the whole startup cost of using a
/// grammar, paid on every run of a tool without it.
///
/// Beware measuring this in a debug build. Cranelift is compiled unoptimised
/// there and the same load takes about four seconds, which is a fact about
/// the profile rather than about the runtime.
///
/// The key covers the wasm bytes and the host. It does NOT cover the runtime
/// version: wasmer writes its own into the artifact and
/// [`Module::deserialize_from_file`] -- the checked reader, unlike
/// `deserialize`, which does not validate -- refuses one it did not write.
/// An entry that has gone stale that way fails to load, is deleted here, and
/// is rebuilt. Adding a version to the key would duplicate a check that
/// already exists, using a number this crate has no reliable way to read.
///
/// Set TREEBANK_NO_COMPILE_CACHE=1 to skip it.
fn compile_cached(store: &Store, bytes: &[u8]) -> Result<Module> {
    if std::env::var_os("TREEBANK_NO_COMPILE_CACHE").is_some() {
        return Module::new(store, bytes).context("not a valid wasm module");
    }

    let key = {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher.update(std::env::consts::ARCH.as_bytes());
        hasher.update(std::env::consts::OS.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let path = compile_cache_dir().join(format!("{}.cwasm", &key[..32]));

    if path.is_file() {
        // SAFETY: loading an artifact is loading executable code, so the
        // bytes must be ones this runtime wrote. `deserialize_from_file`
        // validates the header it wrote and returns Err rather than
        // misbehaving when it did not, which is why a failure here falls
        // through to compiling instead of propagating. The unchecked
        // variants, and `deserialize` itself, make no such promise.
        if let Ok(module) = unsafe { Module::deserialize_from_file(store, &path) } {
            return Ok(module);
        }
        let _ = std::fs::remove_file(&path);
    }

    // Compile ONCE, then keep the artifact. The module returned here is the
    // one that gets used; serialising is only how the next process avoids
    // repeating the work.
    let module = Module::new(store, bytes).context("not a valid wasm module")?;

    // Best effort: a read-only or full cache directory must not stop a parse,
    // so a failure to store is ignored. Written through a temporary file in
    // the same directory and renamed, because two processes compiling the
    // same grammar at once is the ordinary case for a build and a
    // half-written artifact would be read as a whole one.
    if let Ok(artifact) = module.serialize() {
        if let Some(dir) = path.parent() {
            if std::fs::create_dir_all(dir).is_ok() {
                let tmp = dir.join(format!(".{}.cwasm", std::process::id()));
                if std::fs::write(&tmp, &artifact).is_ok() {
                    let _ = std::fs::rename(&tmp, &path);
                }
            }
        }
    }
    Ok(module)
}

fn compile_cache_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("TREEBANK_CACHE") {
        return std::path::PathBuf::from(dir).join("compiled");
    }
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return std::path::PathBuf::from(dir)
            .join("treebank")
            .join("compiled");
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home)
            .join(".cache")
            .join("treebank")
            .join("compiled");
    }
    std::env::temp_dir().join("treebank").join("compiled")
}
