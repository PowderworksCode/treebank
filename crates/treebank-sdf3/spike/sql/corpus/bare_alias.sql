-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16 mysql/5.6 mysql/5.7 mysql/8.0 mariadb/10.11
SELECT id n, qty * 2 AS twice FROM t;
