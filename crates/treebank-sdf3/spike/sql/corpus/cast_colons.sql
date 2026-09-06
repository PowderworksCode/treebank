-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16
-- `::` is PostgreSQL's; the standard spelling is CAST(x AS t).
SELECT id::TEXT FROM t;
