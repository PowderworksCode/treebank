-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16 mysql/5.6 mysql/5.7 mysql/8.0 mariadb/10.11
-- Keywords in any case, the same tree as the upper-case spelling.
select id, name from t where qty > 1 and not (name = 'x') order by id desc limit 2;
Insert Into t (id, name) Values (3, 'c');
