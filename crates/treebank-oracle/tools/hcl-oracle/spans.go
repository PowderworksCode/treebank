// Node BOUNDARIES from hclsyntax, for `treebank shape`.
//
// The validity oracle answers one bit per file and is structurally blind to
// the other kind of defect — a file that parses cleanly into the WRONG
// tree. hclsyntax already builds a tree and the verdict throws it away;
// this keeps the byte extents from it so the sweep can check that every
// boundary the reference parser sees is a boundary we have too.
//
// `hclsyntax.Walk` is the whole traversal: every node in the package
// implements `Range()`, so no per-type visitor is needed and nothing here
// has to be revisited when the library grows a node type.
//
// Five node types are DROPPED, all of them because their range is not a
// boundary in the file:
//
//   - `Attributes` and `Blocks` are GROUPING constructs — a map and a
//     slice — and the library says so itself: "an Attributes doesn't
//     really have a useful range to report [...] we'll arbitrarily take
//     the range of one of the attributes". Over a map, "arbitrarily" means
//     a different one per run, so keeping them would make this oracle's
//     output nondeterministic for no information at all.
//   - `ChildScope` wraps an expression that binds names, and its range is
//     exactly that expression's.
//   - `ObjectConsKeyExpr` wraps an object key so the parser can decide
//     later whether a bare identifier is a name or a variable; its range
//     is exactly its child's too.
//   - `AnonSymbolExpr` is the implicit "each element" of a splat. In
//     `a.*.b` it is the value `.b` is read from, and its range is the
//     zero-width point after the `*`. There is nothing at that offset to
//     match.
//
// Three token types are skipped in the lexical stream, and the reasoning
// is CPython's oracle's: a boundary is only worth comparing where a tree
// could have a node. `TokenNewline` and `TokenComment` are trivia the
// grammar routes through `extras` — the newline is a real terminator and a
// hidden token, so its extent exists in the parse and not in the tree —
// and `TokenEOF` marks no text at all. py-oracle/spans.py drops
// NEWLINE/NL/INDENT/DEDENT/ENCODING/ENDMARKER and COMMENT for the same
// reason. Every delimiter that DOES have a node — the quotes, the heredoc
// markers — stays in, which is what the check is for.
//
// Field EDGES are not reported. hclsyntax's nodes are Go structs with no
// generic field reflection, so naming an edge means a hand-written mapping
// from Go struct fields to this grammar's field names — the correspondence
// table the spans check exists to avoid. `has_edges: false` says so
// explicitly rather than letting an empty list claim every file has none,
// which is the same answer javac's oracle gives.
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"

	"github.com/hashicorp/hcl/v2"
	"github.com/hashicorp/hcl/v2/hclsyntax"
)

type spanFile struct {
	Path     string   `json:"path"`
	Spans    [][3]any `json:"spans"`
	HasEdges bool     `json:"has_edges"`
	Tokens   [][2]int `json:"tokens"`
	Error    *int     `json:"error,omitempty"`
	Skipped  string   `json:"skipped,omitempty"`
}

// hclsyntax.Walk wants a Walker; collecting is all this one does.
// See the note above: trivia and bookends, not boundaries.
var lexSkip = map[hclsyntax.TokenType]struct{}{
	hclsyntax.TokenNewline: {},
	hclsyntax.TokenComment: {},
	hclsyntax.TokenEOF:     {},
}

type collector struct {
	spans [][3]any
}

func (c *collector) Enter(node hclsyntax.Node) hcl.Diagnostics {
	switch node.(type) {
	case hclsyntax.Attributes, hclsyntax.Blocks,
		hclsyntax.ChildScope, *hclsyntax.ChildScope,
		*hclsyntax.ObjectConsKeyExpr, *hclsyntax.AnonSymbolExpr:
		return nil
	}
	r := node.Range()
	if r.End.Byte <= r.Start.Byte {
		return nil
	}
	c.spans = append(c.spans, [3]any{r.Start.Byte, r.End.Byte, fmt.Sprintf("%T", node)})
	return nil
}

func (c *collector) Exit(hclsyntax.Node) hcl.Diagnostics { return nil }

func spansMain() {
	in := bufio.NewScanner(os.Stdin)
	in.Buffer(make([]byte, 0, 64*1024), 4*1024*1024)
	out := bufio.NewWriter(os.Stdout)
	defer out.Flush()

	for in.Scan() {
		path := in.Text()
		if path == "" {
			continue
		}
		src, err := os.ReadFile(path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "hcl-oracle: %v\n", err)
			os.Exit(1)
		}
		emit(out, path, src)
	}
	if err := in.Err(); err != nil {
		fmt.Fprintf(os.Stderr, "hcl-oracle: read paths: %v\n", err)
		os.Exit(1)
	}
}

func emit(out *bufio.Writer, path string, src []byte) {
	file := spanFile{Path: path, Spans: [][3]any{}, Tokens: [][2]int{}}

	// A file the reference parser rejects has no tree to compare against.
	// Reporting WHERE it first objected is still useful — rejecting the
	// right files at the wrong offset makes error recovery useless
	// downstream — so the offset goes out and the spans do not.
	parsed, diags := hclsyntax.ParseConfig(src, path, hcl.InitialPos)
	if diags.HasErrors() {
		for _, d := range diags {
			if d.Severity == hcl.DiagError && d.Subject != nil {
				offset := d.Subject.Start.Byte
				file.Error = &offset
				break
			}
		}
		file.Skipped = "hclsyntax rejected the file"
		write(out, file)
		return
	}

	body, ok := parsed.Body.(*hclsyntax.Body)
	if !ok {
		file.Skipped = "not a native-syntax body"
		write(out, file)
		return
	}

	// An EMPTY slice, never nil: a file with nothing in it (or with only
	// comments) has no spans, and Go marshals a nil slice as `null`, which
	// the reader cannot tell from "this oracle reports no spans at all".
	c := &collector{spans: [][3]any{}}
	hclsyntax.Walk(body, c)
	file.Spans = c.spans

	// The lexical oracle rides along, the role `tokenize` plays for python:
	// hclsyntax exposes its scanner separately from its parser, so token
	// extents cost one more call over bytes already in hand.
	tokens, tokDiags := hclsyntax.LexConfig(src, path, hcl.InitialPos)
	if !tokDiags.HasErrors() {
		for _, t := range tokens {
			if _, skip := lexSkip[t.Type]; skip {
				continue
			}
			r := t.Range
			if r.End.Byte > r.Start.Byte {
				file.Tokens = append(file.Tokens, [2]int{r.Start.Byte, r.End.Byte})
			}
		}
	}

	write(out, file)
}

func write(out *bufio.Writer, file spanFile) {
	encoded, err := json.Marshal(file)
	if err != nil {
		fmt.Fprintf(os.Stderr, "hcl-oracle: %v\n", err)
		os.Exit(1)
	}
	out.Write(encoded)
	out.WriteByte('\n')
}
