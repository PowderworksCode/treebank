-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16 mysql/5.6 mysql/5.7 mysql/8.0 mariadb/10.11
INSERT INTO t (id, name, qty) VALUES (1, 'a', 2 * 3);
UPDATE t SET qty = qty + 1 WHERE id = 1;
DELETE FROM t WHERE id = 1;
