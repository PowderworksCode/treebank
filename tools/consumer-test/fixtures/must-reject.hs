-- Rejected by GHC (GHC-58481, parse error on input `=`) and by the grammar.
module MustReject where

f x = = = x +
data = 3
