// Syntax-only Go validity check for the treebank oracle.
//
// stdin:  one file path per line
// stdout: "<path>\tvalid|invalid" per line
//
// The reference parser is Go's own, `go/parser.ParseFile` — the parser
// gofmt, go vet and the toolchain's own tooling run. It parses and stops:
// no type checking, no import resolution, no package assembly, so an
// unresolved identifier or a missing dependency is not an error and each
// file is judged entirely on its own text. That is the same property that
// makes ts.createSourceFile usable for TypeScript, JavacTask.parse() for
// Java and compile(..., 'exec') for Python.
//
// SkipObjectResolution turns off building the deprecated ast.Object /
// ast.Scope graph, which is the one part of ParseFile that tries to link
// identifiers to declarations. Skipping it is both faster and *more*
// honest for a single file with no package context: resolution across a
// package's other files is exactly the information we do not have.
//
// Errors are ALL syntax errors. go/parser has no notion of a semantic
// diagnostic, so there is no severity filter to get wrong here, unlike
// Roslyn or javac.
package main

import (
	"bufio"
	"go/parser"
	"go/token"
	"os"
	"strings"
)

func parses(path string) bool {
	fset := token.NewFileSet()
	// src == nil makes the parser read the file itself; an unreadable file
	// comes back as an error and is reported invalid, matching the other
	// oracles' handling of an I/O failure.
	_, err := parser.ParseFile(fset, path, nil, parser.SkipObjectResolution)
	return err == nil
}

func main() {
	in := bufio.NewScanner(os.Stdin)
	in.Buffer(make([]byte, 0, 1<<20), 1<<20)
	out := bufio.NewWriter(os.Stdout)
	defer out.Flush()
	for in.Scan() {
		path := strings.TrimSpace(in.Text())
		if path == "" {
			continue
		}
		verdict := "invalid"
		if parses(path) {
			verdict = "valid"
		}
		out.WriteString(path)
		out.WriteByte('\t')
		out.WriteString(verdict)
		out.WriteByte('\n')
	}
}
