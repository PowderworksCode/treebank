// The HCL oracle: one Go program, two modes.
//
//	hcl-oracle           -- validity.  stdout: "<path>\tvalid|invalid" per line
//	hcl-oracle spans     -- boundaries. stdout: one JSON object per line
//
// stdin is one file path per line in both modes.
//
// The judgment is `hclsyntax.ParseConfig`, which is the HCL native-syntax
// parser itself — the same call OpenTofu's `fmt` gates on before it will
// touch a file, and the same one Terraform's loader makes before any of its
// own work begins. It resolves no provider, reads no module, checks no
// type and needs no `terraform init`: a file is judged on its own bytes.
// That is exactly the line this grammar is measured against, and it is
// enforced by the library's own boundary rather than by our discipline.
//
// An UNREADABLE file is never an invalid file. A read error exits non-zero
// with no verdict, because an `invalid` verdict books a file the grammar
// failed as corpus noise — an oracle that answered `invalid` for files it
// could not open would convert every grammar failure into noise and report
// a flawless grammar.
package main

import (
	"bufio"
	"fmt"
	"os"

	"github.com/hashicorp/hcl/v2"
	"github.com/hashicorp/hcl/v2/hclsyntax"
)

// The sentinel `stdin_oracle::Persistent` writes to mark the end of a
// batch, echoed back so one process can serve many batches. `fuzz` asks
// one question at a time and would otherwise spend its run starting
// processes.
const sentinel = "\x00--end--"

func main() {
	if len(os.Args) > 1 && os.Args[1] == "spans" {
		spansMain()
		return
	}
	in := bufio.NewScanner(os.Stdin)
	// Corpus paths are short, but a caller is free to pass long ones; the
	// default 64 KiB token limit is about the LINE, not the file.
	in.Buffer(make([]byte, 0, 64*1024), 4*1024*1024)
	out := bufio.NewWriter(os.Stdout)
	defer out.Flush()

	for in.Scan() {
		path := in.Text()
		if path == "" {
			continue
		}
		if path == sentinel {
			fmt.Fprintln(out, sentinel)
			out.Flush()
			continue
		}
		src, err := os.ReadFile(path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "hcl-oracle: %v\n", err)
			os.Exit(1)
		}
		verdict := "valid"
		if _, diags := hclsyntax.ParseConfig(src, path, hcl.InitialPos); diags.HasErrors() {
			verdict = "invalid"
		}
		fmt.Fprintf(out, "%s\t%s\n", path, verdict)
	}
	if err := in.Err(); err != nil {
		fmt.Fprintf(os.Stderr, "hcl-oracle: read paths: %v\n", err)
		os.Exit(1)
	}
}
