module Patched (
	eval,
	where',
) where

-- Patch 0003: a tab-indented line whose first token starts with one of the
-- letters the scanner probes for layout keywords (w i t e d m). Without the
-- patch the tab itself parses as (ERROR (UNEXPECTED '\t')).
data' :: Int
data' = 1

-- Patch 0004: a let inside a tab-indented do block, which needs the tab to
-- count as one column so the layout context matches an interior column.
eval :: IO Int
eval = do
	r <- pure 1
	let c = r + 1
	pure c

where' :: Int
where' = 2
