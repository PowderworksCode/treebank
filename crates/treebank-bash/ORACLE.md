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

## Exit codes are the verdict, and there are more than two of them

| exit | meaning | verdict |
|---|---|---|
| 0 | parsed | valid |
| 2 | syntax error, almost everywhere | invalid |
| **1** | **syntax error inside an array-assignment word list** | invalid |
| 126 | bash refuses the file (NUL byte, directory) | **not a verdict** |
| 127 | file does not exist | **not a verdict** |

The `1` row is the one worth knowing about, and it was measured rather than
anticipated. `x=( a+([0-9]) )` exits **1**; the same extglob pattern outside
an array (`echo a+([0-9])`, `case $y in a+([0-9])) ;; esac`) exits **2**. So
does every other syntax error probed — an unterminated string, a stray
`esac`, a missing `then`. Any error inside the parentheses of an array
assignment takes the other exit: `x=( ;; )` and `x=( a` also exit 1.

This is not a curiosity in a test file. linux ships
`tools/testing/selftests/wireguard/netns.sh`, which is exactly that
construct, and it is the *only* file in 67,586 across both corpora that
exits 1 — one file, which with `2` alone in the reject list would abort the
entire sweep.

126 and 127 are deliberately **not** verdicts. That is the property
`exec_oracle` is built around: if every non-zero status meant "invalid", a
mistyped corpus root would score every file invalid, every failing file
would be recorded as corpus noise, `gap_files` would fall to zero and the
sweep would report a flawless grammar. A broken oracle has to fail loudly
rather than quietly agree with us. `lang/bash.rs`'s `admit()` scans the
**whole** file for a NUL — not a leading window — precisely so that a 126
cannot reach the oracle; measured over both corpora, no admitted file
contains one.

## Dialect

`tree-sitter.json` declares `first-line-regex: ^#!.*\b(sh|bash|dash)\b.*$`,
and the corpus admits exactly what that claims. Pointing this oracle at zsh,
ksh, fish or csh would turn the grammar's *correct* rejection of another
language into a reported gap — the same trap as pointing the JavaScript
oracle at the TypeScript parser.

The other case the shebang cannot catch is the **template**. A file that
*renders to* a shell script is not one, and `bash -n` cannot see the
difference: ComplianceAsCode ships `.sh` files beginning
`{{% if product in ['sle15'] %}}`, and those tags lex as ordinary shell
words, so the oracle calls them **valid**. They therefore arrive as grammar
*gaps* rather than as noise — the one direction in which a two-valued
per-file oracle can inflate the very number it exists to protect. Measured
before it was fixed: 419 of 1,388 GitHub gap files, 30%.

Nothing in the oracle can fix that, so the corpus does it instead:
`lang/bash.rs`'s `admit()` drops a file carrying a Jinja/Django *statement*
tag. See the ledger's `corpus.template_filter` for why it is anchored on a
keyword — a bare `{%…%}` rule has an 80% false-positive rate on Debian,
where `{%s%}` is a gettext format-string fixture.

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
file. So the fix is the one the ROADMAP already prescribes for php — run the forks
in parallel — and this oracle does not implement that itself. It calls
`lang::exec_oracle`, the shared fork-per-file driver the PHP session landed,
whose own note says "the next fork-per-file oracle should inherit it by
calling it". Bash is that next one. It contributed one generalization back:
`reject_status` became `reject_statuses`, a list, because of the exit-1 row
above.

Measured through the sweep on the GitHub corpus, 2,205 files adjudicated,
`TREEBANK_ORACLE_JOBS` varied:

| workers | whole sweep |
|---|---|
| 1 | 7.30 s |
| 4 | 3.28 s |
| 16 (this box's core count, the default) | 2.25 s |

The sweep's own parsing is a fixed ~2.0 s of that, so the oracle itself goes
from ~5.3 s to ~0.25 s — the same 2.4 → 0.11 s per thousand as the table
above, now with no shell script and no `xargs` in the path.

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
