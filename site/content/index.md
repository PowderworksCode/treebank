---
title: Treebank
description: Tree-sitter grammars, with the evidence attached.
---

<p class="cover cover-wide"><img src="/cover.png" alt="A plate of fifteen trees drawn to scale, each with a small figure beside it for size" width="560"></p>

Treebank owns nine tree-sitter grammars — bash, C, C++, Java, Python, Ruby,
Rust, TypeScript and Zig — and treats each one as a claim that has to be paid
for. The claim is that the grammar accepts the language. The payment is a
corpus of real code, a reference parser to adjudicate against, and a committed
ledger saying what was measured and when.

Most grammar projects report a pass rate. A pass rate answers one question —
does it accept valid code? — and is silent on the two that matter just as
much: does it *reject* what the language rejects, and when it accepts, does it
build the right tree? Treebank measures all three, separately, and publishes
what it finds including where it is losing.

## What is here

**[The grammar reference](/grammars/)** renders every production in every
grammar, as EBNF and as a railroad diagram, straight from the parse table.
1,570 productions across the nine. It is generated from `src/grammar.json` —
the normalised grammar tree-sitter itself consumes — so it cannot drift from
the parser: if a production is on the page, the parse table has it.

**[How it works](/concepts/)** is the machinery: the two tiers, the oracles,
the sweeps, and why evidence is committed rather than reported.

**[Reference](/reference/)** is the CLI, twenty-one commands, and the gates
each grammar has to pass.
