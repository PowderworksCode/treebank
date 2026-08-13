%% One construct per grammar patch in crates/treebank-erlang.
%%
%% Patch 0003 — macro bodies that are not expressions or complete clauses. A
%% -define body only has to make sense where it is EXPANDED, and upstream's
%% replacement rule required a form, an expression or a whole clause. All four
%% shapes below are valid Erlang the compiler accepts (verified with epp) and
%% came out of a Hex sweep: telemetry and epgsql ship the first, brod the
%% second, jsone the third.
-module(patched).

-define(WITH_STACKTRACE(T, R, S), T:R:S ->).
-define(BIND_STACKTRACE(Var), :Var).
-define(OPT, #decode_opt_v2).
-define(CATCH(T, R, S), T:R -> S = erlang:get_stacktrace(), ).
