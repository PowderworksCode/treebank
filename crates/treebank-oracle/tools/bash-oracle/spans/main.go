// Node BOUNDARIES for bash, for `treebank shape`.
//
// stdin:  one file path per line
// stdout: one JSON object per line:
//         {"path":..., "spans":[[start,end,"Kind"],...], "error":off, "skipped":...}
//
// AUTHORITY, stated plainly: this is not bash. bash has no AST to ask for
// -- `bash -n` reports a verdict and nothing else -- so the boundaries
// come from mvdan.cc/sh, an independent reimplementation, and this check
// is differential against a PEER, not against the reference. Where the
// two trees disagree, neither is automatically right; a disagreement is a
// place for a human to look, and the sweep's validity verdicts still come
// from bash itself. javac and CPython carry more weight in their oracles
// than mvdan/sh carries here, and shape_policy.toml entries for bash say
// which reading was chosen and why.
//
// Offsets are BYTES: mvdan/sh's Pos.Offset is a byte offset already, so
// unlike javac there is nothing to convert.
package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"reflect"
	"strings"

	"mvdan.cc/sh/v3/syntax"
)

type out struct {
	Path    string           `json:"path"`
	Spans   [][3]interface{} `json:"spans"`
	Error   *int             `json:"error,omitempty"`
	Skipped string           `json:"skipped,omitempty"`
	// mvdan/sh has no generic field reflection over its AST either.
	HasEdges bool `json:"has_edges"`
}

func main() {
	parser := syntax.NewParser(syntax.KeepComments(false), syntax.Variant(syntax.LangBash))
	in := bufio.NewScanner(os.Stdin)
	in.Buffer(make([]byte, 1024*1024), 1024*1024)
	w := bufio.NewWriter(os.Stdout)
	defer w.Flush()
	enc := json.NewEncoder(w)
	for in.Scan() {
		path := strings.TrimSpace(in.Text())
		if path == "" {
			continue
		}
		rec := out{Path: path, Spans: [][3]interface{}{}}
		src, err := os.ReadFile(path)
		if err != nil {
			// An unreadable file is an oracle FAILURE, never a verdict.
			fmt.Fprintf(os.Stderr, "bash-oracle: cannot read %s: %v\n", path, err)
			os.Exit(1)
		}
		f, err := parser.Parse(strings.NewReader(string(src)), path)
		if err != nil {
			rec.Skipped = "parse"
			if pe, ok := err.(syntax.ParseError); ok {
				off := int(pe.Pos.Offset())
				if off <= len(src) {
					rec.Error = &off
				}
			}
			enc.Encode(rec)
			continue
		}
		syntax.Walk(f, func(node syntax.Node) bool {
			if node == nil {
				return false
			}
			s, e := node.Pos().Offset(), node.End().Offset()
			if !node.Pos().IsValid() || !node.End().IsValid() || e <= s || int(e) > len(src) {
				return true
			}
			kind := reflect.TypeOf(node).Elem().Name()
			// File is the whole-input wrapper, our `program`; Word and
			// Lit are below the granularity our _word_like model keeps.
			if kind == "File" {
				return true
			}
			rec.Spans = append(rec.Spans, [3]interface{}{s, e, kind})
			return true
		})
		enc.Encode(rec)
	}
}
