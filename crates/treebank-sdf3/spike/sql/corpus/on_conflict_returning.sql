-- targets: postgres/9.5 postgres/12 postgres/15 postgres/16
INSERT INTO t (id) VALUES (1) ON CONFLICT DO NOTHING RETURNING id;
