%% Invalid Erlang by syntax alone — both OTP front ends reject it (epp_dodger
%% and epp alike), and so must the grammar. Patch 0003 widened what a macro
%% BODY may be; it must not have widened what a call may be, and an unclosed
%% argument list is the nearest miss.
-module(must_reject).

f() -> g(1, 2.
