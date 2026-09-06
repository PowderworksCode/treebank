-- targets: postgres/15 postgres/16
-- PostgreSQL 15 added MERGE.
MERGE INTO t USING s ON t.id = s.id
  WHEN MATCHED THEN UPDATE SET qty = s.qty
  WHEN NOT MATCHED THEN INSERT (id, name) VALUES (s.id, s.name);
