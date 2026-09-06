-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16 mysql/8.0 mariadb/10.11
-- MySQL took common table expressions in 8.0, MariaDB in 10.2.
WITH c AS (SELECT id FROM t WHERE qty > 0) SELECT id FROM c;
