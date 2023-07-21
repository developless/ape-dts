DROP DATABASE IF EXISTS twoway_test_db_1;

CREATE DATABASE IF NOT EXISTS twoway_test_db_1;

CREATE TABLE IF NOT EXISTS twoway_test_db_1.one_pk_no_uk ( f_0 tinyint, f_1 smallint DEFAULT NULL, f_2 mediumint DEFAULT NULL, f_3 int DEFAULT NULL, f_4 bigint DEFAULT NULL, f_5 decimal(10,4) DEFAULT NULL, f_6 float(6,2) DEFAULT NULL, f_7 double(8,3) DEFAULT NULL, f_8 bit(64) DEFAULT NULL, f_9 datetime(6) DEFAULT NULL, f_10 time(6) DEFAULT NULL, f_11 date DEFAULT NULL, f_12 year DEFAULT NULL, f_13 timestamp(6) NULL DEFAULT NULL, f_14 char(255) DEFAULT NULL, f_15 varchar(255) DEFAULT NULL, f_16 binary(255) DEFAULT NULL, f_17 varbinary(255) DEFAULT NULL, f_18 tinytext, f_19 text, f_20 mediumtext, f_21 longtext, f_22 tinyblob, f_23 blob, f_24 mediumblob, f_25 longblob, f_26 enum('x-small','small','medium','large','x-large') DEFAULT NULL, f_27 set('a','b','c','d','e') DEFAULT NULL, f_28 json DEFAULT NULL, PRIMARY KEY (f_0) ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4; 

TRUNCATE TABLE twoway_test_db_1.one_pk_no_uk;

DROP DATABASE IF EXISTS ape_trans_mysql;
CREATE DATABASE ape_trans_mysql;

CREATE TABLE ape_trans_mysql.topo1_node1_to_node2 (n int) ENGINE=InnoDB;
insert into ape_trans_mysql.topo1_node1_to_node2 set n = 0;

CREATE TABLE ape_trans_mysql.topo1_node3_to_node2 (n int) ENGINE=InnoDB;
insert into ape_trans_mysql.topo1_node3_to_node2 set n = 0;