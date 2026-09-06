-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16 mysql/5.6 mysql/5.7 mysql/8.0 mariadb/10.11
-- Every target accepts this line and the trees disagree: in PostgreSQL
-- "name" is a quoted identifier (the column), in MySQL it is a string.
SELECT "name" FROM t;
