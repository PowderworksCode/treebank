#!/usr/bin/env python3
# Syntax-only YAML validity check against the libyaml lineage.
#
# stdin:  one file path per line
# stdout: "<path>\tvalid|invalid" per line
#
# This is the SECOND implementation family, not a second version. PyYAML is
# a reimplementation of libyaml's state machine and shares its readings, and
# between them they are what the installed base actually runs: PyYAML, the
# C libyaml bindings, Ruby's Psych, Go's gopkg.in/yaml and everything built
# on those. They are more permissive than the spec in places and stricter in
# others, and a file the whole Python and Ruby world reads without complaint
# is a file this grammar had better parse.
#
# `yaml.parse` runs the PARSER and stops there: it yields events and never
# composes, resolves a tag or constructs an object. That is deliberate and
# it is the same choice the zig oracle makes in preferring `zig fmt` over
# `ast-check` — the readier an oracle is to say "invalid", the more flawless
# the grammar looks, because an invalid verdict books our failure as corpus
# noise. `yaml.safe_load` would reject unresolvable aliases and duplicate
# keys, which are resolution errors this grammar is right not to see.
import sys

import yaml

try:
    from yaml import CSafeLoader as Loader  # libyaml itself where present
except ImportError:  # pragma: no cover - depends on the local build
    from yaml import SafeLoader as Loader


def valid(path):
    # An unreadable file is NOT an invalid file: see check.mjs for why that
    # distinction is the difference between a broken oracle and a flawless
    # grammar. Exit loudly instead.
    try:
        with open(path, "rb") as handle:
            source = handle.read()
    except OSError as exc:
        sys.stderr.write("yaml-oracle: cannot read %s: %s\n" % (path, exc))
        sys.exit(1)
    try:
        for _ in yaml.parse(source, Loader=Loader):
            pass
        return True
    except Exception:
        return False


for line in sys.stdin:
    path = line.rstrip("\n")
    if not path:
        continue
    sys.stdout.write("%s\t%s\n" % (path, "valid" if valid(path) else "invalid"))
    sys.stdout.flush()
