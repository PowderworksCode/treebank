module M where
data C = C { a :: Int }
f C{..} = a
