-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16
SELECT * FROM t WHERE name ILIKE 'a%';
