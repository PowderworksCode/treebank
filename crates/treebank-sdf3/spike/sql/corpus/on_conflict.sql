-- targets: postgres/9.5 postgres/12 postgres/15 postgres/16
-- PostgreSQL 9.5 added INSERT ... ON CONFLICT.
INSERT INTO t (id, name) VALUES (1, 'a') ON CONFLICT (id) DO UPDATE SET qty = 2;
INSERT INTO t (id) VALUES (1) ON CONFLICT DO NOTHING;
