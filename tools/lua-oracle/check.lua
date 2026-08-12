-- Syntax-only Lua validity check for the treebank oracle.
--
-- stdin:  one file path per line
-- stdout: "<path>\tvalid|invalid" per line
--
-- The reference parser is PUC-Rio Lua's own, driven through
-- `loadfile(path, "t")` — the exact call Lua makes to turn a file's text
-- into a callable chunk. It compiles and stops: it never runs the chunk,
-- never resolves a `require`, never touches a global. A file that opens a
-- file, shells out and prints is judged valid without any of that
-- happening (verified: neither side-effect file exists afterwards), the
-- same property that makes ts.createSourceFile usable for TypeScript and
-- compile() for Python.
--
-- Why loadfile and not `luac -p`, which is the obvious choice and the one
-- the roadmap costed. Two reasons, one measured and one about correctness.
--
--   Measured. `luac -p` has no batch mode, so it forks per file. On a
--   1000-file sample (10.4 MB of real Lua) that is 1.65 s, of which ~1.15 s
--   is process creation — measured directly by running the same 1000 forks
--   over an empty file. Batching through one interpreter is 0.17 s: ~10x,
--   with no parallel driver to build. `xargs -P16` gets to 0.48 s, so the
--   in-process form still wins by 3x against the parallelised fork.
--
--   Correctness. `loadfile` is luaL_loadfilex — the same entry point luac
--   itself uses — so it inherits luac's handling of a leading `#!` line.
--   `load(<string>)` does not skip that line and would call every
--   shebanged script invalid. Verified verdict-for-verdict against
--   `luac -p` over 2606 real files from six Lua projects: identical, 2601
--   valid / 5 invalid, zero disagreements.
--
-- The one deliberate divergence from `luac -p` is mode "t", which refuses
-- precompiled binary chunks that luac would load and call valid. A bytecode
-- blob has no Lua syntax in it, so the grammar correctly fails to parse it;
-- calling it valid would manufacture a grammar gap out of a file that has
-- no source at all. Measured occurrence in the 2606-file sample: zero.
--
-- THE INTERPRETER VERSION IS THE LANGUAGE VERSION, and for Lua that is not
-- a detail — it is the whole blocker. `goto` is 5.2+, integer division and
-- bitwise operators are 5.3+, `<const>`/`<close>` are 5.4, LuaJIT adds
-- 64-bit (`1LL`) and binary (`0b1010`) literals that no PUC Lua accepts,
-- and Luau is a different language again. Whichever `luac` is installed
-- decides the verdicts. So this refuses to run under the wrong one rather
-- than silently producing verdicts for a dialect nobody recorded; the
-- pinned version lives in crates/treebank-lua/ledger.json's `oracle` field,
-- and bumping it is treated like a patch: full sweep, before/after numbers,
-- ledger entry.
local WANT = "Lua 5.4"
if _VERSION ~= WANT then
  io.stderr:write(string.format(
    "lua-oracle: refusing to run under %s; this oracle is pinned to %s.\n" ..
    "  The Lua version IS the dialect (goto is 5.2+, // is 5.3+, <const> is 5.4),\n" ..
    "  so verdicts from another interpreter would not mean what ledger.json says.\n" ..
    "  Install it (apt install lua5.4) or update crates/treebank-lua/ledger.json's\n" ..
    "  oracle field together with a fresh sweep.\n", _VERSION, WANT))
  os.exit(1)
end

-- An unreadable file is NOT an invalid file. Returning "invalid" here looks
-- harmless and is not: validate() is only ever called on files the grammar
-- already failed, and an invalid verdict records the file as corpus NOISE.
-- So a mistyped corpus root would make every path unreadable, every grammar
-- failure noise, gap_files zero — and the sweep would report a flawless
-- grammar. A broken oracle must fail loudly, never quietly agree with us
-- (the reasoning is spelled out in crates/treebank-cli/src/lang/exec_oracle.rs).
--
-- Lua makes this trap easier to fall into than the other oracles, because
-- `loadfile` returns the SAME nil for "this is not valid Lua" and "I could
-- not open that". So readability is established separately, with an explicit
-- open and read, before the verdict is asked for.
--
-- The verdict itself still comes from `loadfile` rather than from
-- `load(<the bytes we just read>)`, deliberately: loadfile is
-- luaL_loadfilex, the entry point `luac -p` uses, and it skips a leading
-- `#!` line where load(<string>) does not. Reusing the bytes would save a
-- second open and would silently change what this oracle means — the
-- equivalence with `luac -p` is measured (2606 corpus files and a 20-file
-- adversarial battery, zero disagreements) and it is measured about
-- loadfile.
local function must_be_readable(path)
  local f, err = io.open(path, "rb")
  if not f then
    io.stderr:write(
      ("lua-oracle: cannot read %s: %s\n"):format(path, err or "unknown error"),
      "lua-oracle: this is an oracle failure, not a verdict; check the corpus root\n")
    os.exit(1)
  end
  -- A directory opens cleanly on Linux and only fails on read, so the read
  -- is part of the check rather than the open alone.
  local ok, content = pcall(f.read, f, "a")
  f:close()
  if not ok or content == nil then
    io.stderr:write(
      ("lua-oracle: cannot read %s: %s\n"):format(path, (not ok) and tostring(content) or "read returned nothing"),
      "lua-oracle: this is an oracle failure, not a verdict; check the corpus root\n")
    os.exit(1)
  end
end

local out = io.stdout
for line in io.lines() do
  local path = line:match("^%s*(.-)%s*$")
  if path ~= "" then
    must_be_readable(path)
    -- Second return value (the message) is deliberately dropped: a file is
    -- valid or it is not, and the reason is the sweep's business, not the
    -- oracle's. By here the file is known readable, so a nil chunk is a
    -- verdict about its CONTENT.
    local chunk = loadfile(path, "t")
    out:write(path, "\t", chunk and "valid" or "invalid", "\n")
  end
end
out:flush()
