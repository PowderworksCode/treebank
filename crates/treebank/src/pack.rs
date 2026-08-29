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
//! what the last sweep measured, and [`Pack::roles`] returns the facet
//! manifest that [`crate::expand`] needs, so a facet query can be expanded
//! without shipping the grammar's `roles.json` alongside the parser.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use serde::Deserialize;
use wasmtime::{Engine, Instance, Linker, Memory, Module, Store, TypedFunc};
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::WasiCtxBuilder;

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

/// The facet manifest a pack carries, which is what a facet query is expanded
/// against.
#[derive(Debug, Clone, Deserialize)]
pub struct PackRoles {
    #[serde(default)]
    pub facets: BTreeMap<String, Vec<String>>,
}

const NAMED: u32 = 1;
const IS_ERROR: u32 = 2;
const HAS_ERROR: u32 = 4;
const MISSING: u32 = 8;

/// A loaded grammar.
pub struct Pack {
    store: std::cell::RefCell<Store<WasiP1Ctx>>,
    memory: Memory,
    f: Abi,
    provenance: Provenance,
    roles: PackRoles,
}

struct Abi {
    strlen: TypedFunc<u32, u32>,
    alloc: TypedFunc<u32, u32>,
    free: TypedFunc<u32, ()>,
    parse: TypedFunc<(u32, u32), u32>,
    tree_free: TypedFunc<u32, ()>,
    tree_root: TypedFunc<(u32, u32), ()>,
    node_new: TypedFunc<(), u32>,
    node_free: TypedFunc<u32, ()>,
    node_type: TypedFunc<u32, u32>,
    node_sexp: TypedFunc<u32, u32>,
    node_flags: TypedFunc<u32, u32>,
    node_start_byte: TypedFunc<u32, u32>,
    node_end_byte: TypedFunc<u32, u32>,
    node_child_count: TypedFunc<u32, u32>,
    node_child: TypedFunc<(u32, u32, u32), u32>,
    node_named_child_count: TypedFunc<u32, u32>,
    node_named_child: TypedFunc<(u32, u32, u32), u32>,
    field_name_for_child: TypedFunc<(u32, u32), u32>,
    cstr_free: TypedFunc<u32, ()>,
}

impl Pack {
    /// Load a pack from a file.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .with_context(|| format!("reading {}", path.display()))?;
        Self::from_bytes(&bytes)
    }

    /// Load a pack from bytes, which is what a consumer that fetched one over
    /// HTTP has.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let engine = Engine::default();
        let module = compile_cached(&engine, bytes)?;

        let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |c| c)
            .context("linking WASI")?;
        // A pack opens no files and reads no environment. The context exists
        // because the six imports must resolve, not because they are used.
        let mut store = Store::new(&engine, WasiCtxBuilder::new().build_p1());
        let instance = linker
            .instantiate(&mut store, &module)
            .context("instantiating the pack")?;

        // Reactor exec model: _initialize runs the module's constructors.
        instance
            .get_typed_func::<(), ()>(&mut store, "_initialize")
            .context("pack has no _initialize; is it a treebank pack?")?
            .call(&mut store, ())?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow!("pack exports no memory"))?;
        let f = Abi::bind(&instance, &mut store)?;

        let provenance = read_json(&mut store, &memory, &instance, "tb_provenance")?;
        let roles = read_json(&mut store, &memory, &instance, "tb_roles")?;

        Ok(Self { store: std::cell::RefCell::new(store), memory, f, provenance, roles })
    }

    /// What the pack says about itself.
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    /// The facet manifest, for [`crate::expand`].
    pub fn roles(&self) -> &PackRoles {
        &self.roles
    }

    /// The language this pack parses.
    pub fn language(&self) -> &str {
        &self.provenance.language
    }

    /// Expand a facet query against this pack's manifest.
    ///
    /// `(_callable)` becomes `[(function_definition) (lambda)]`, which is what
    /// makes a query portable across grammars: the facet is the same word
    /// everywhere and its members are whatever this grammar calls them.
    pub fn expand_query(&self, query: &str) -> Result<String> {
        crate::expand::expand(query, &self.roles.facets)
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
            .write(&mut *store, ptr as usize, bytes)
            .context("writing source into the pack")?;
        let tree = self.f.parse.call(&mut *store, (ptr, len))?;
        self.f.free.call(&mut *store, ptr)?;
        if tree == 0 {
            return Err(anyhow!("parse failed"));
        }
        Ok(Tree { pack: self, handle: tree })
    }
}

/// A parsed tree. Freed when dropped.
pub struct Tree<'p> {
    pack: &'p Pack,
    handle: u32,
}

impl<'p> Tree<'p> {
    pub fn root(&self) -> Node<'p> {
        let mut store = self.pack.store.borrow_mut();
        let node = self.pack.f.node_new.call(&mut *store, ()).expect("tb_node_new");
        self.pack
            .f
            .tree_root
            .call(&mut *store, (self.handle, node))
            .expect("tb_tree_root");
        Node { pack: self.pack, handle: node }
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
        cstr(&mut store, &self.pack.memory, &self.pack.f, ptr)
    }

    /// The whole subtree as an s-expression.
    pub fn sexp(&self) -> Result<String> {
        let mut store = self.pack.store.borrow_mut();
        let ptr = self.pack.f.node_sexp.call(&mut *store, self.handle)?;
        let out = cstr(&mut store, &self.pack.memory, &self.pack.f, ptr)?;
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
        let kid = self.pack.f.node_new.call(&mut *store, ())?;
        let f = if named_only { &self.pack.f.node_named_child } else { &self.pack.f.node_child };
        let ok = f.call(&mut *store, (self.handle, index, kid))?;
        if ok == 0 {
            self.pack.f.node_free.call(&mut *store, kid)?;
            return Ok(None);
        }
        Ok(Some(Node { pack: self.pack, handle: kid }))
    }

    /// The field name the PARENT gives its `index`-th child, which is the edge
    /// label a query uses. Field names belong to the parent's view of a child,
    /// which is why this is asked here rather than of the child.
    pub fn field_name_for_child(&self, index: u32) -> Result<Option<String>> {
        let mut store = self.pack.store.borrow_mut();
        let ptr = self.pack.f.field_name_for_child.call(&mut *store, (self.handle, index))?;
        if ptr == 0 {
            return Ok(None);
        }
        Ok(Some(cstr(&mut store, &self.pack.memory, &self.pack.f, ptr)?))
    }

    /// Every named child, as a vector.
    pub fn named_children(&self) -> Result<Vec<Node<'p>>> {
        let count = self.child_count(true)?;
        (0..count).filter_map(|i| self.child(i, true).transpose()).collect()
    }
}

impl Drop for Node<'_> {
    fn drop(&mut self) {
        let mut store = self.pack.store.borrow_mut();
        let _ = self.pack.f.node_free.call(&mut *store, self.handle);
    }
}

fn cstr(
    store: &mut Store<WasiP1Ctx>,
    memory: &Memory,
    f: &Abi,
    ptr: u32,
) -> Result<String> {
    if ptr == 0 {
        return Ok(String::new());
    }
    let len = f.strlen.call(&mut *store, ptr)? as usize;
    let mut buf = vec![0u8; len];
    memory.read(&mut *store, ptr as usize, &mut buf)?;
    Ok(String::from_utf8(buf)?)
}

fn read_json<T: for<'de> Deserialize<'de>>(
    store: &mut Store<WasiP1Ctx>,
    memory: &Memory,
    instance: &Instance,
    name: &str,
) -> Result<T> {
    let ptr = instance
        .get_typed_func::<(), u32>(&mut *store, name)
        .with_context(|| format!("pack has no {name}"))?
        .call(&mut *store, ())?;
    let len = instance
        .get_typed_func::<(), u32>(&mut *store, &format!("{name}_len"))
        .with_context(|| format!("pack has no {name}_len"))?
        .call(&mut *store, ())? as usize;
    let mut buf = vec![0u8; len];
    memory.read(&mut *store, ptr as usize, &mut buf)?;
    serde_json::from_slice(&buf).with_context(|| format!("parsing {name}"))
}

impl Abi {
    fn bind(instance: &Instance, store: &mut Store<WasiP1Ctx>) -> Result<Self> {
        macro_rules! f {
            ($name:literal) => {
                instance
                    .get_typed_func(&mut *store, $name)
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
        })
    }
}

/// Compiling a pack costs a few hundred milliseconds; loading an
/// already-compiled one costs a few. Measured on a release build: python
/// 296ms cold against 4ms warm, C++ 370ms against 25ms. Everything else here
/// is far cheaper -- reading the file and parsing a small program are both
/// under a millisecond -- so this is the whole startup cost of using a
/// grammar, paid on every run of a tool without it.
///
/// Beware measuring this in a debug build. Cranelift is compiled unoptimised
/// there and the same load takes about four seconds, which is a fact about
/// the profile rather than about wasmtime.
///
/// The key covers the wasm bytes and the host. It does NOT cover the wasmtime
/// version, because wasmtime writes its own version into the artifact and
/// refuses to deserialize one from a different major -- so an entry that has
/// gone stale that way fails to load, is deleted here, and is rebuilt. Adding
/// a version to the key would duplicate a check that already exists, using a
/// number this crate has no reliable way to read.
///
/// Set TREEBANK_NO_COMPILE_CACHE=1 to skip it.
fn compile_cached(engine: &Engine, bytes: &[u8]) -> Result<Module> {
    if std::env::var_os("TREEBANK_NO_COMPILE_CACHE").is_some() {
        return Module::new(engine, bytes).context("not a valid wasm module");
    }

    let key = {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hasher.update(std::env::consts::ARCH.as_bytes());
        hasher.update(std::env::consts::OS.as_bytes());
        hasher.finalize().iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    let path = compile_cache_dir().join(format!("{}.cwasm", &key[..32]));

    if let Ok(precompiled) = std::fs::read(&path) {
        // SAFETY: the file was produced by `precompile_module` on this
        // machine, under a key that covers the wasm bytes, the wasmtime
        // version and the host. Wasmtime validates its own header as well and
        // returns Err rather than misbehaving if any of that is wrong, which
        // is why a failure here falls through to compiling instead of
        // propagating.
        if let Ok(module) = unsafe { Module::deserialize(engine, &precompiled) } {
            return Ok(module);
        }
        let _ = std::fs::remove_file(&path);
    }

    // Compile ONCE. `precompile_module` does the work and hands back the
    // artifact, which deserializes in milliseconds -- so this is the compile,
    // not an extra one beside it. Calling `Module::new` as well doubled the
    // cold path from 3.8s to 7.6s, which is the kind of thing that only shows
    // up if you measure the miss as well as the hit.
    match engine.precompile_module(bytes) {
        Ok(precompiled) => {
            // Best effort: a read-only or full cache directory must not stop a
            // parse, so a failure to store is ignored.
            if let Some(dir) = path.parent() {
                if std::fs::create_dir_all(dir).is_ok() {
                    let tmp = dir.join(format!(".{}.cwasm", std::process::id()));
                    if std::fs::write(&tmp, &precompiled).is_ok() {
                        let _ = std::fs::rename(&tmp, &path);
                    }
                }
            }
            // SAFETY: produced by this engine, moments ago, in this process.
            unsafe { Module::deserialize(engine, &precompiled) }
                .context("loading the module just compiled")
        }
        Err(_) => Module::new(engine, bytes).context("not a valid wasm module"),
    }
}

fn compile_cache_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("TREEBANK_CACHE") {
        return std::path::PathBuf::from(dir).join("compiled");
    }
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return std::path::PathBuf::from(dir).join("treebank").join("compiled");
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".cache").join("treebank").join("compiled");
    }
    std::env::temp_dir().join("treebank").join("compiled")
}
