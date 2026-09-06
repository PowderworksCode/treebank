-- targets: mysql/5.6 mysql/5.7 mariadb/10.11
-- MySQL 8.0 removed the query cache and SQL_CACHE with it; MariaDB kept both.
-- `*` follows the hint so that no target can read SQL_CACHE as a column
-- with a bare alias, which PostgreSQL otherwise would.
SELECT SQL_CACHE * FROM t;
