-- targets: mysql/5.6 mysql/5.7 mysql/8.0 mariadb/10.11
INSERT INTO t (id, name) VALUES (1, 'a') ON DUPLICATE KEY UPDATE qty = 2;
