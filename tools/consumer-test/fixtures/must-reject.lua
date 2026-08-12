-- Invalid in EVERY Lua dialect (verified against 5.1.5, 5.4.6 and LuaJIT
-- 2.1), so this tests the grammar's strictness rather than the dialect gap.
local function f()
  return 1
