-- targets: postgres/9.4 postgres/9.5 postgres/12 postgres/15 postgres/16
-- ... and kept WITHOUT OIDS as a no-op.
CREATE TABLE u (id INT) WITHOUT OIDS;
