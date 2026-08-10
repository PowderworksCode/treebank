#!/usr/bin/env bash
. "$HOME/.cargo/env"
export PATH="$HOME/.local/bin:$HOME/.bun/bin:$PATH"
unset GH_HOST
cd "$HOME/treebank"
exec claude --remote-control tbcron --permission-mode auto "$(cat "$HOME/treebank/CRON-BRIEF.md")"
