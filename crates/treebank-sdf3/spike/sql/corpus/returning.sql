-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16
INSERT INTO t (id, name) VALUES (2, 'b') RETURNING id;
UPDATE t SET qty = 1 WHERE id = 2 RETURNING id, qty;
DELETE FROM t WHERE id = 2 RETURNING *;
