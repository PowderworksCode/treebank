-- No grammar patch: this crate is upstream at the pinned sha. What this
-- fixture pins instead is the DIALECT the ledger claims — Lua 5.x *plus*
-- LuaJIT 2.x, upstream's stated target and the basis for the sweep's
-- gap/noise split. If an upstream bump ever narrowed it, the numbers in
-- ledger.json would quietly stop meaning what they say; this catches it.
--
-- Deliberately, NO single interpreter accepts this whole file: `<const>` is
-- 5.4 and LuaJIT rejects it, while `1LL` and `0b1010` are LuaJIT and every
-- PUC Lua rejects them. That is the point — the grammar is the union of the
-- dialects, which is exactly why ledger.json has to record which one the
-- oracle is. See crates/treebank-lua/LOCAL-PATCHES.md.
local t <const> = 42          -- 5.4 attrib
local n = 7 // 2              -- 5.3 integer division
local b = 1 | 2 & 3 ~ 4 >> 5  -- 5.3 bitwise
local big = 0xFFULL           -- LuaJIT 64-bit literal
local wide = 1LL              -- LuaJIT 64-bit literal
local bin = 0b1010            -- LuaJIT binary literal, in no PUC Lua
local s = [[a long string, ]=] inside it is just text]]
local z = "joined \z
           across lines"      -- 5.2 \z escape

for i = 1, 3 do
  if i == 2 then goto continue end   -- 5.2 goto
  print(t, n, b, big, wide, bin, s, z)
  ::continue::
end
