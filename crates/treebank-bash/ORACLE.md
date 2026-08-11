# The Bash validity oracle

Answer to the first-move question, written before any grammar work:
**does `bash -n` execute anything, and what does its verdict actually
assert?** For every other language in treebank the oracle is a parser
library. Here it is a shell, pointed at thousands of strangers' scripts,
and being wrong about the first question is not a recoverable mistake.

Every number and every verdict below was produced on this machine
(GNU bash 5.2.21(1), Linux 6.12.93, 16 cores) before a single corpus file
was fetched.

## What the oracle asserts

> **Claim.** `bash` reads this entire file and builds a command tree from
> it without a syntax error.

It deliberately does **not** claim:

- that the script *runs*. Nothing is executed, so an undefined variable, a
  missing command, a bad `cd` and a failed `source` are all invisible here.
- that the script is *correct*. `bash -n` is a parser, and shell's parser
  is permissive by design.
- that arithmetic or expansions are well-formed. Those are evaluated at
  runtime, not parsed: `$(( 1 + + ))` and `${!}` are both **valid** to this
  oracle and both fail when run. An unterminated heredoc is likewise a
  runtime warning, not a parse error.
- that the file is valid in some other shell. This is bash, so it is the
  reference parser for bash and for the POSIX-sh subset bash accepts —
  and for nothing else. See "Dialect" below.

The third point bounds what the oracle may be used for: it can adjudicate
whether a file the grammar rejected is really invalid, but it can never be
evidence for **tightening** the grammar inside arithmetic or expansions,
because it does not look there either.

## It does not execute — verified, not trusted

`perl -c` runs `BEGIN` blocks. That is the failure mode this section exists
to rule out. Each probe below is a real file, run through `bash -n`, with a
real canary: a `~/CANARY_DIR` containing a file, and `touch` targets in the
working directory.

| probe | file contents | result |
|---|---|---|
| missing include | `source /absent/file` · `. /also/absent` | **exit 0** — not an error |
| destructive command | `rm -rf "$HOME/CANARY_DIR"` | exit 0, directory and contents **intact** |
| command substitution | `X=$(touch CANARY)` · `` Y=`touch CANARY` `` · `${Z:=$(touch CANARY)}` | no file created |
| process substitution | `diff <(touch CANARY) >(touch CANARY)` · `exec 3< <(touch CANARY)` | no file created |
| heredoc body | `cat <<EOF` / `$(touch CANARY)` / `EOF` | no file created |
| eval | `eval "touch CANARY"` | no file created |
| sourcing at top level | `. ./substitution.sh` (a file that touches canaries) | no file created |
| startup files | `BASH_ENV=./evil.sh bash -n x.sh`, same for `ENV` | no file created |

Zero canaries fired across all eight. The first row is the one the ROADMAP
names, and the third through seventh are the ones that would have made the
first row a false comfort — a shell that expanded `$(...)` while parsing
would run arbitrary code from any script in the corpus.

Hostile *inputs* are contained too, which matters because a corpus fetched
from a distribution contains files that are not scripts:

| input | result |
|---|---|
| 200 KB of `/dev/urandom` | exit 2 (syntax error) in ~15 ms |
| file with an embedded NUL | exit **126**, `cannot execute binary file` |
| an ELF binary (`/bin/ls`) | exit **126** |
| a directory | exit 126, `Is a directory` |
| a path that does not exist | exit **127** |

## Exit codes are the verdict, and there are more than two

| exit | meaning | verdict |
|---|---|---|
| 0 | parsed | valid |
| 2 | syntax error | invalid |
| 126 | bash refuses the file (NUL byte, directory) | invalid — bash will not run it, so it is not a valid script |
| 127 | file does not exist | **not a verdict** — a harness bug |

`check.sh` tests for a readable regular file first and complains on stderr,
so a 127 can never be silently recorded as "invalid". 126 is kept as a real
verdict; `lang/bash.rs`'s `admit()` drops NUL-bearing files at corpus-build
time anyway, so it should be unreachable from a sweep.

## Dialect

`tree-sitter.json` declares `first-line-regex: ^#!.*\b(sh|bash|dash)\b.*$`,
and the corpus admits exactly what that claims. Pointing this oracle at zsh,
ksh, fish or csh would turn the grammar's *correct* rejection of another
language into a reported gap — the same trap as pointing the JavaScript
oracle at the TypeScript parser.

The one case the shebang cannot catch is the polyglot: a `#!/bin/sh` preamble
that `exec`s another interpreter on itself. netpbm ships ten of them
(`pnmflip`, `ppmfade`, `pgmcrater`, …) — two lines of shell, then Perl. They
enter the corpus, the oracle calls them invalid, and they are recorded as
noise. That is the right bucket, but it is not a free one.

## Cost: the fork is the oracle

Measured over 963 shell scripts from this machine's `/usr` and `/etc`
(6.5 MB, mean 6.2 KB), found by shebang scan plus `*.sh`/`*.bash`.

| measurement | s / 1000 | per file |
|---|---|---|
| `/bin/true` × 1000 — bare process spawn | 0.63–0.80 | 0.7 ms |
| `bash -c :` × 1000 — bash startup, no file | 1.55–1.65 | 1.6 ms |
| `bash -n` on an **empty** file × 1000 | 1.95–2.06 | 2.0 ms |
| **`bash -n` over the corpus, serial** | **2.28–2.53** | **2.4 ms** |
| the same at `xargs -P16` | 0.41–0.51 | — |
| 10,000 files at `-P16`, `-n 64` | **0.115** | — |
| `dash -n` over the same corpus (control) | 1.16 | 1.2 ms |

**The ROADMAP records 3.6 s/1000; this machine measures 2.4.** The
discrepancy is not a methodology disagreement, and the decomposition says
why: **83% of the cost is process startup and only ~0.4 ms/file is parsing**
(≈65 MB/s of shell text). Both figures are therefore mostly a measurement of
`fork+exec` on their respective hardware, and the part that is a property of
bash — 0.4 ms — is consistent with each.

The consequential correction is the classification, not the digit.
**`bash -n` belongs in the ROADMAP's fork-per-file class with `php -l`**, not
with the batch oracles it is tabled beside. There is no batch escape: bash
cannot syntax-check a file from inside a long-lived shell, because `set -n`
stops that shell from executing the very `source` that would read the next
file. So the fix is the one the ROADMAP already prescribes for php — run the
forks in parallel — and `check.sh` does, which is why it is the second
parallel oracle in the repo after rust's.

## Honesty: 0 false rejects

The same 963 local scripts, adjudicated: **952 valid, 11 invalid, 0 false
rejects.** All 11 were read and are genuinely not bash:

- ten netpbm polyglots (`sub pm_message($) {`, `my @spline10 = (…)`) — Perl;
- `/usr/share/doc/ucf/examples/postinst`, a documentation example whose
  `: if test ! -d /usr/local/lib/foo; then` really is a bash syntax error.

## Permissiveness, probed

23 constructs, to find where the oracle stops looking. It correctly rejects
unclosed command substitutions, unbalanced function braces, a stray `done`
or `esac`, a `case` with no `esac`, an unclosed `$((`, an unterminated
string, `echo >` and `echo |`, an empty `f() { }`, a `select` with no `do`,
`[[ a -foo b ]]`, an unclosed `[[`, a zsh anonymous function and a Perl
polyglot. It accepts `function f { }`, `declare -A`, `<(…)` and `coproc`,
all of which are real bash.

The three it accepts that a *runtime* would reject are listed under "What the
oracle asserts": `$(( 1 + + ))`, `${!}`, and an unterminated heredoc.
