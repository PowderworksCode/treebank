-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16 mysql/5.7 mysql/8.0
-- `->>` came to MySQL in 5.7 and never to MariaDB; PostgreSQL has had it since 9.3.
SELECT j->>'$.a' FROM t WHERE j->>'$.b' = 'x';
