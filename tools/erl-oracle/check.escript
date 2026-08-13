#!/usr/bin/env escript
%%! -noshell
%%
%% Syntax-only Erlang validity check for the treebank oracle.
%%
%% argv:   the corpus source root (bounds the include-path walk below)
%% stdin:  one file path per line
%% stdout: "<path>\tvalid|invalid" per line
%%
%% ============================================================ THE ORACLE
%%
%% A UNION of OTP's two front ends, because neither one can judge Erlang on
%% its own and the roadmap's single-parser plan does not survive contact with
%% a corpus:
%%
%%   epp_dodger  parses WITHOUT the preprocessor. Macros are dodged rather
%%               than expanded, so no include is needed and every file is
%%               judged on its own text. This is the tool the roadmap picked,
%%               and it is why Erlang is Tier A where C is Tier B.
%%
%%   epp         the real preprocessor plus parser: expands macros, follows
%%               -include and -include_lib, and therefore needs the file's
%%               project around it.
%%
%% A file is VALID if EITHER accepts it. Measured over 1,715 real files from
%% 130 Hex packages, and the two fail in almost disjoint directions:
%%
%%   dodger alone   1,610 valid  -- 105 rejects (6.1%)
%%   epp alone      1,601 valid  -- 114 rejects (6.6%)
%%   UNION          1,706 valid  --   9 rejects (0.52%)
%%
%%   105 files: dodger accepts, epp cannot (an -include_lib on a sibling
%%              package that is not on any path we can build)
%%    96 files: epp accepts, dodger cannot (see below)
%%
%% WHY DODGER ALONE IS NOT ENOUGH, which is the finding here. epp_dodger
%% parses a -define's BODY as if it were a form, and a macro body does not
%% have to be one -- it only has to make sense where it is expanded. Real
%% headers are full of bodies that are not forms:
%%
%%     -define(IS_ETAGC(C), C =:= 16#21; C >= 16#23, C =/= 16#7f).  % cowlib
%%     -define(BIND_STACKTRACE(Var), :Var).                        % brod
%%     -define(CFMT(C, Col), f([$~,$!,C|S]) -> [Col|f(S)];         % cf
%%                           f([$~,$!,$_,C|S]) -> [Col,?U|f(S)]).
%%
%% All three are valid Erlang -- epp compiles them -- and epp_dodger reports
%% a syntax error for each. Using it alone would book 6% of the corpus as
%% "invalid", i.e. as corpus NOISE, which is the direction that HIDES grammar
%% gaps rather than inventing them. The roadmap's "epp_dodger, blocker: none,
%% the preprocessor problem with a happy ending" is half right: the ending is
%% happy because dodger removes the *include* dependency, not because dodger
%% alone can parse Erlang.
%%
%% ====================================================== THE OTHER TRAP
%%
%% epp_dodger:parse_file/1 returns {ok, Forms} FOR A FILE WITH SYNTAX ERRORS.
%% The errors arrive as `error` FORMS inside the list, so the obvious reading
%%
%%     case epp_dodger:parse_file(F) of {ok,_} -> valid; _ -> invalid end
%%
%% is an oracle that calls every broken file valid -- every grammar failure
%% becomes noise, gap_files goes to zero, and the sweep reports a flawless
%% grammar. It is the "never ship a validate() that returns everything is
%% valid" failure, and it is the DEFAULT reading of this API. Both parsers
%% are therefore checked the same way: scan the forms.
%%
%% And a third, found by feeding it 4 KB of /dev/urandom: on input the
%% scanner cannot even tokenize, parse_file returns {ok, []} -- zero forms,
%% no error. So an empty form list from a file with any non-whitespace byte
%% in it is a REJECT, not a pass. (A genuinely blank file is valid and stays
%% valid; measured occurrences in the corpus: zero either way.)
%%
%% The top-level {error, _} means something else again, and here the split is
%% the exact inverse of Lua's trap: it is ONLY I/O -- {0,file,enoent} for a
%% missing path, {0,file,eisdir} for a directory. So it is an ORACLE FAILURE,
%% exits non-zero, and never becomes a verdict: validate() runs only on files
%% the grammar already failed, so a mistyped corpus root would otherwise turn
%% every failure into noise and report a perfect grammar.

main([SrcRoot]) ->
    ok = io:setopts(standard_io, [binary]),
    check_otp(),
    loop(filename:absname(SrcRoot));
main(_) ->
    io:format(standard_error, "usage: check.escript <corpus-src-root> < paths~n", []),
    halt(2).

%% The OTP release is the dialect. Erlang's syntax moves slowly but it moves:
%% maps arrived in 17, stacktrace-binding try/catch in 21, the `maybe`
%% expression in 25 behind a feature flag and 27 by default. Which OTP parses
%% the corpus decides what "invalid" means, exactly as the interpreter version
%% does for Lua. Pinned in crates/treebank-erlang/ledger.json's oracle field.
check_otp() ->
    Want = "28",
    case erlang:system_info(otp_release) of
        Want -> ok;
        Other ->
            io:format(standard_error,
                      "erl-oracle: refusing to run under OTP ~s; this oracle is pinned to OTP ~s.~n"
                      "  The OTP release IS the dialect, so verdicts from another one would not~n"
                      "  mean what crates/treebank-erlang/ledger.json says its sweep numbers mean.~n"
                      "  Install the pinned toolchain (tools/beam-toolchain/fetch.sh --otp-only)~n"
                      "  or update the ledger's oracle field together with a fresh sweep.~n",
                      [Other, Want]),
            halt(1)
    end.

die(Path, Reason) ->
    io:format(standard_error,
              "erl-oracle: cannot read ~ts: ~p~n"
              "erl-oracle: this is an oracle failure, not a verdict; check the corpus root~n",
              [Path, Reason]),
    halt(1).

loop(Root) ->
    case io:get_line(standard_io, "") of
        eof -> ok;
        {error, R} -> die("<stdin>", R);
        Line ->
            case string:trim(binary_to_list(Line)) of
                "" -> ok;
                Path -> io:format("~ts\t~s~n", [Path, verdict(Path, Root)])
            end,
            loop(Root)
    end.

verdict(Path, Root) ->
    %% Readability is established first and separately, so that an I/O
    %% failure can never be mistaken for a parse failure by either parser.
    Bytes = case file:read_file(Path) of
                {ok, B} -> B;
                {error, R} -> die(Path, R)
            end,
    case blank(Bytes) of
        true -> "valid";
        false ->
            case decodes(Path, Bytes) andalso (dodger(Path) orelse epp(Path, Root)) of
                true -> "valid";
                false -> "invalid"
            end
    end.

%% An encoding gate in front of the union, because epp_dodger does not have
%% one and the union would inherit that. Erlang source is UTF-8 unless the
%% file says otherwise in a `%% coding:` comment, which epp:read_encoding
%% reads for us -- the same call the compiler makes. epp rejects bytes that
%% do not decode; dodger accepts them, so without this the union would too,
%% and a file that is not text in any encoding would be scored valid and any
%% grammar failure on it recorded as a gap that does not exist.
%%
%% Measured on 1,715 corpus files before adopting it: 1,650 declare no
%% encoding (UTF-8 by default), 65 declare utf8, none declare latin-1, and
%% every one of them decodes -- so this rejects nothing that is real. A
%% latin-1 declaration is honoured rather than second-guessed, because
%% latin-1 source is still legal Erlang and any byte sequence is valid
%% latin-1.
decodes(Path, Bytes) ->
    case epp:read_encoding(Path) of
        latin1 -> true;
        _ -> is_list(unicode:characters_to_list(Bytes, utf8))
    end.

blank(Bytes) ->
    string:is_empty(string:trim(binary_to_list(Bytes))).

dodger(Path) ->
    try epp_dodger:parse_file(Path) of
        {error, Reason} -> die(Path, Reason);
        %% Zero forms from a non-blank file: the scanner gave up. Not a pass.
        {ok, []} -> false;
        {ok, Forms} -> not lists:any(fun is_error_form/1, Forms)
    catch
        %% Narrow on purpose. Random bytes make the tokenizer raise, and that
        %% is a verdict about content. An oracle BUG must crash, not vote, so
        %% nothing wider is caught here.
        error:_ -> false; exit:_ -> false; throw:_ -> false
    end.

%% epp needs the file's project around it. There is no build system to ask,
%% so this walks every directory from the file's own up to the corpus root
%% and offers each one plus its include/ and src/. That finds a package root
%% at whatever depth the file sits -- prometheus keeps modules three levels
%% down in src/collectors/vm/ -- without hardcoding any registry's layout.
%% Measured: the walk resolves exactly as many files as passing the true
%% package root does (9 residual rejects either way), where a fixed
%% two-levels-up guess leaves 19.
epp(Path, Root) ->
    try epp:parse_file(Path, include_dirs(Path, Root), []) of
        {error, _} -> false;              % I/O here is dodger's to report
        {ok, Forms} -> not lists:any(fun is_error_form/1, Forms)
    catch
        error:_ -> false; exit:_ -> false; throw:_ -> false
    end.

include_dirs(Path, Root) ->
    Depth = length(filename:split(Root)),
    Walk = fun Walk(D, Acc) ->
                   case length(filename:split(D)) =< Depth of
                       true -> [Root | Acc];
                       false -> Walk(filename:dirname(D), [D | Acc])
                   end
           end,
    lists:flatmap(fun(D) -> [D, filename:join(D, "include"), filename:join(D, "src")] end,
                  Walk(filename:dirname(Path), [])).

is_error_form({error, _}) -> true;
is_error_form(_) -> false.
