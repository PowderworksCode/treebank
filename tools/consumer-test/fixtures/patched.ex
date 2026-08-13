# One construct per grammar patch in crates/treebank-elixir.
#
# Patch 0003 — a backslash pair immediately before the end delimiter of a
# NON-interpolating string or sigil. Upstream's scanner reads the second
# backslash as the start of a new escape, so `~S(\\)` parses as `\` followed
# by an escaped `)` and the sigil never terminates. Found by the corpus
# sweep in telemetry_metrics_prometheus_core, which escapes backslashes for
# Prometheus exposition text.
#
# Every line below is valid Elixir 1.20 (verified with
# Code.string_to_quoted/2) and must parse Clean.
defmodule Treebank.Patched do
  def escape(value) do
    value
    |> to_string()
    |> String.replace(~S(\\), ~S(\\\\))
    |> String.replace(~S[\\], ~S{\\})
    |> String.replace(~S"\\", ~S/\\/)
  end
end
