-module(a).
-define(GEN(N), N() -> ok.).
?GEN(f).
