# Syntax-only Elixir validity check for the treebank oracle.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# The reference parser is Elixir's own front end, `Code.string_to_quoted/2`
# — the exact call that turns a file's text into an AST before the compiler
# does anything else with it. It tokenizes and parses and stops: no macro is
# expanded, no `use`/`import`/`require` is resolved, no module attribute is
# evaluated, no `.exs` statement is run. Every file is judged on its own
# text, so a missing dependency is not an error.
#
# THAT IT DOES NOT EVALUATE IS VERIFIED, NOT ASSUMED, because Elixir runs
# arbitrary code at COMPILE time and this is pointed at thousands of
# strangers' files. Six adversarial fixtures — a module body calling
# File.write! and System.cmd, a module attribute bound to File.read!, a
# defmacro plus a module that does `use`/`import`/`require` on it, a
# top-level .exs script running System.cmd, :os.cmd, Code.eval_string and
# spawn, sigil and string interpolation wrapping a side effect, and
# compile-time `if`/`for` — were parsed through this path: zero files
# written, zero processes spawned, zero subprocesses, BEAM process count
# unchanged. The control is what makes that mean anything: COMPILING and
# RUNNING those same six files produces ten side-effect markers. The battery
# is not vacuous, it just never fires here.
#
# WHY A BATCH ORACLE, WHICH IS NOT THE OBVIOUS IMPLEMENTATION. The obvious
# one is a fork per file, the way `php -l` and `bash -n` must work. Measured
# on this corpus: `elixir -e` per file is 31.2 s for 100 files, i.e. **312 s
# per 1000** — worse than libclang's 35.5 and a third of the way to C++'s
# 1068, which would put the roadmap's easiest Tier-A oracle outside Tier A
# altogether. The BEAM boots in ~0.2 s and Elixir adds ~0.3 s on top, so a
# fork-per-file design throws away 0.49 s per FILE. Batching through one
# long-lived VM costs that 0.49 s once: 0.96 s per 1000 files end to end,
# 0.65 s per 1000 excluding startup, 9.4 MB/s. There is no parallel driver
# to write and nothing to tune. Tier A here is a property of the
# implementation, not of the language.
#
# THE ELIXIR VERSION IS THE DIALECT. Elixir's parser changes between minor
# releases, so which `elixir` runs this decides what "invalid" means, the
# same way the interpreter version does for Lua and the compiler version
# does for Zig. This refuses to run under an unpinned minor rather than
# silently producing verdicts for a dialect nobody recorded; the pin lives
# in crates/treebank-elixir/ledger.json's `oracle` field, and bumping it is
# treated like a patch: full sweep, before/after numbers, ledger entry.
want_minor = "1.20"
otp_want = "28"

unless String.starts_with?(System.version(), want_minor <> ".") do
  IO.write(:stderr, """
  elixir-oracle: refusing to run under Elixir #{System.version()}; this oracle is pinned to #{want_minor}.x.
    The Elixir version IS the dialect, so verdicts from another one would not mean
    what crates/treebank-elixir/ledger.json says its sweep numbers mean.
    Install the pinned toolchain (tools/beam-toolchain/fetch.sh) or update the
    ledger's oracle field together with a fresh sweep.
  """)
  System.halt(1)
end

# OTP is a warning rather than a refusal, and the asymmetry is deliberate:
# the parser is Elixir code, so the Elixir version decides the grammar, while
# OTP underneath it supplies the Unicode tables and the runtime. A mismatch
# there is worth saying out loud and is not on its own a reason to refuse.
otp = :erlang.system_info(:otp_release) |> to_string()

if otp != otp_want do
  IO.write(:stderr, "elixir-oracle: warning: running on OTP #{otp}, ledger pins OTP #{otp_want}\n")
end

defmodule TreebankOracle do
  # An unreadable file is NOT an invalid file. `validate()` is only ever
  # called on files the grammar already failed, and an `invalid` verdict
  # records the file as corpus NOISE — so a mistyped corpus root would turn
  # every grammar failure into noise, drive gap_files to zero, and report a
  # flawless grammar. A broken oracle must fail loudly, never quietly agree
  # with us. (The reasoning is spelled out in
  # crates/treebank-cli/src/lang/exec_oracle.rs.)
  defp die(msg) do
    IO.write(:stderr, "elixir-oracle: #{msg}\nelixir-oracle: this is an oracle failure, not a verdict; check the corpus root\n")
    System.halt(1)
  end

  defp read!(path) do
    case File.read(path) do
      {:ok, bin} -> bin
      # A directory opens cleanly on Linux and fails here with :eisdir, which
      # is exactly the shape of a mistyped root, so it must be fatal too.
      {:error, reason} -> die("cannot read #{path}: #{:file.format_error(reason)}")
    end
  end

  # `Code.string_to_quoted/2` returns `{:error, _}` for every syntax and
  # token error, so the verdict is a match on the tag. The rescue is
  # deliberately ONE exception wide.
  #
  # UnicodeConversionError is the only thing measured to escape as an
  # exception, raised on bytes that are not UTF-8 — and that is a verdict
  # about content, since Elixir source is UTF-8 by definition. Everything
  # else was checked and comes back as `{:error, _}`: hand-written garbage,
  # a lone NUL byte, an unterminated string, an unterminated heredoc, a byte
  # order mark (which `elixirc` also rejects — verified, so this is not a
  # divergence), 20,000-deep parenthesis nesting and a 50,000-element list.
  #
  # A blanket `rescue -> "invalid"` is what this must not have, and the
  # reason is not hypothetical: the first draft of this file passed
  # `column: true` where the option is spelled `columns:`. That raises
  # FunctionClauseError deep in :elixir_compiler, the blanket rescue caught
  # it, and the oracle cheerfully reported all 3,359 corpus files invalid —
  # which as a sweep result means every grammar failure is noise, gap_files
  # is zero, and the grammar looks perfect. An oracle bug must crash, not
  # vote.
  defp verdict(src, path) do
    case Code.string_to_quoted(src, file: path) do
      {:ok, _} -> "valid"
      {:error, _} -> "invalid"
    end
  rescue
    UnicodeConversionError -> "invalid"
  end

  def run do
    IO.stream(:stdio, :line)
    |> Stream.map(&String.trim/1)
    |> Stream.reject(&(&1 == ""))
    |> Stream.each(fn path -> IO.write([path, ?\t, verdict(read!(path), path), ?\n]) end)
    |> Stream.run()
  end
end

TreebankOracle.run()
