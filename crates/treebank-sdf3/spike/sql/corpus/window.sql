-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16 mysql/8.0 mariadb/10.11
SELECT id, ROW_NUMBER() OVER (PARTITION BY name ORDER BY id) FROM t;
