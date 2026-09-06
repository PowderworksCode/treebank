-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16 mysql/5.6 mysql/5.7 mysql/8.0 mariadb/10.11
SELECT id, name AS n FROM t WHERE qty > 1 AND NOT (name = 'x' OR qty < 0) ORDER BY id DESC, name;
