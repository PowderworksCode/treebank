// Syntax-only Go validity check for the treebank oracle.
//
// stdin:  one file path per line
// stdout: "<path>\tvalid|invalid" per line, in any order
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
//
// # An unreadable file is not an invalid file
//
// This is the one design point worth stating, and it is borrowed from
// exec_oracle.rs, which reasons it out for the fork-per-file oracles:
// a broken oracle must fail loudly, never quietly agree with us.
//
// Reporting "invalid" for a file we could not read looks harmless and is
// not. validate() is only ever called on files the grammar already failed,
// and an invalid verdict records the file as corpus noise. So a mistyped
// corpus root would make every path unreadable, every grammar failure
// noise, gap_files zero — and the sweep would report a flawless grammar.
// The read is therefore separate from the parse, and an I/O error is fatal
// with the path on stderr. Only a file we actually read can get a verdict.
//
// # Concurrency
//
// Every file is judged on its own text, so this is embarrassingly
// parallel. Paths are read up front and handed to workers one at a time
// rather than in contiguous slices, because corpus files differ in size by
// two orders of magnitude and a static split leaves workers idle behind
// whoever drew the 8 MB file. Output order is therefore not input order;
// stdin_oracle builds a map from it and does not care.
package main

import (
	"bufio"
	"fmt"
	"go/parser"
	"go/token"
	"os"
	"runtime"
	"strconv"
	"strings"
	"sync"
)

// workers mirrors exec_oracle's jobs(): the env var wins, otherwise the
// cores this process may actually use.
func workers() int {
	if s := os.Getenv("TREEBANK_ORACLE_JOBS"); s != "" {
		if n, err := strconv.Atoi(s); err == nil && n > 0 {
			return n
		}
	}
	return runtime.GOMAXPROCS(0)
}

type result struct {
	path  string
	valid bool
}

// parses reports whether the file's text is syntactically valid Go. The
// error return is for I/O only: it means we never reached a verdict.
func parses(path string) (bool, error) {
	src, err := os.ReadFile(path)
	if err != nil {
		return false, err
	}
	_, perr := parser.ParseFile(token.NewFileSet(), path, src, parser.SkipObjectResolution)
	return perr == nil, nil
}

func main() {
	in := bufio.NewScanner(os.Stdin)
	in.Buffer(make([]byte, 0, 1<<20), 1<<20)
	var paths []string
	for in.Scan() {
		if p := strings.TrimSpace(in.Text()); p != "" {
			paths = append(paths, p)
		}
	}
	if err := in.Err(); err != nil {
		fmt.Fprintf(os.Stderr, "go-oracle: reading paths from stdin: %v\n", err)
		os.Exit(1)
	}

	n := workers()
	if n > len(paths) {
		n = len(paths)
	}
	jobs := make(chan string, n)
	results := make(chan result, n)
	var wg sync.WaitGroup
	for i := 0; i < n; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for path := range jobs {
				valid, err := parses(path)
				if err != nil {
					// Fatal, and deliberately so — see the note above.
					fmt.Fprintf(os.Stderr, "go-oracle: cannot read %s: %v\n"+
						"go-oracle: this is an oracle failure, not a verdict; "+
						"check the corpus root\n", path, err)
					os.Exit(1)
				}
				results <- result{path, valid}
			}
		}()
	}
	go func() {
		for _, p := range paths {
			jobs <- p
		}
		close(jobs)
		wg.Wait()
		close(results)
	}()

	out := bufio.NewWriter(os.Stdout)
	defer out.Flush()
	for r := range results {
		verdict := "invalid"
		if r.valid {
			verdict = "valid"
		}
		out.WriteString(r.path)
		out.WriteByte('\t')
		out.WriteString(verdict)
		out.WriteByte('\n')
	}
}
