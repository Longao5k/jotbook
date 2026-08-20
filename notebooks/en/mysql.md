---
name: mysql
description: MySQL / MariaDB - connecting, backup and restore, slow query and lock diagnostics
tags: [db, sql]
vars:
  host:
    desc: host
    from: profile
  db:
    desc: database
    from: profile
  user:
    desc: user
    from: profile
---

## Connect

Never write `-pPASSWORD` on the command line; it lands in your shell history. Use bare -p and let it prompt.

```sh @tags=connect
mysql -h {{host}} -u {{user}} -p {{db}}
```

## Run one statement and exit

```sh @tags=connect
mysql -h {{host}} -u {{user}} -p -e "{{sql}}" {{db}}
```

## Run a SQL file

```sh @tags=connect
mysql -h {{host}} -u {{user}} -p {{db}} < {{file}}
```

## Export a query result as TSV

```sh @tags=connect
mysql -h {{host}} -u {{user}} -p -B -e "{{sql}}" {{db}} > {{file}}.tsv
```

## Client command reference

Use these once inside mysql.

```txt @tags=reference
SHOW DATABASES;              list databases
USE dbname;                  switch database
SHOW TABLES;                 list tables in the current database
DESC tablename;              show the table structure
SHOW CREATE TABLE t\G        show the DDL, indexes and engine included
SHOW INDEX FROM t;           list indexes
SHOW PROCESSLIST;            show connections and running queries
SHOW ENGINE INNODB STATUS\G  InnoDB detail (deadlock info lives here)
STATUS;                      connection info and character set
\G                           print results vertically (essential for wide tables)
\q                           quit
```

## Back up one database

`--single-transaction` gets InnoDB a consistent snapshot without locking tables.

```sh @tags=backup
mysqldump -h {{host}} -u {{user}} -p --single-transaction --routines --triggers {{db}} > {{file}}.sql
```

## Back up and compress

```sh @tags=backup
mysqldump -h {{host}} -u {{user}} -p --single-transaction {{db}} | gzip > {{file}}.sql.gz
```

## Back up the schema only

```sh @tags=backup
mysqldump -h {{host}} -u {{user}} -p --no-data {{db}} > {{file}}.sql
```

## Back up specific tables

```sh @tags=backup
mysqldump -h {{host}} -u {{user}} -p --single-transaction {{db}} {{table}} > {{file}}.sql
```

## Restore

```sh @tags=backup @confirm
mysql -h {{host}} -u {{user}} -p {{db}} < {{file}}.sql
```

## Restore from a compressed dump

```sh @tags=backup @confirm
gunzip < {{file}}.sql.gz | mysql -h {{host}} -u {{user}} -p {{db}}
```

## See how much space each table uses

```sql @tags=diagnose
SELECT table_name,
       ROUND(data_length  / 1024 / 1024, 1) AS data_mb,
       ROUND(index_length / 1024 / 1024, 1) AS index_mb,
       table_rows
FROM information_schema.tables
WHERE table_schema = DATABASE()
ORDER BY data_length + index_length DESC
LIMIT 20;
```

## See the total size of each database

```sql @tags=diagnose
SELECT table_schema AS db,
       ROUND(SUM(data_length + index_length) / 1024 / 1024, 1) AS size_mb
FROM information_schema.tables
GROUP BY table_schema
ORDER BY size_mb DESC;
```

## Show currently running queries

```sql @tags=diagnose
SELECT id, user, host, db, command, time, state, LEFT(info, 200) AS query
FROM information_schema.processlist
WHERE command <> 'Sleep'
ORDER BY time DESC;
```

## Terminate a connection

```sql @tags=diagnose @confirm
KILL {{id}};
```

## Show lock waits

```sql @tags=diagnose
SELECT r.trx_id AS waiting_trx, r.trx_mysql_thread_id AS waiting_thread,
       LEFT(r.trx_query, 100) AS waiting_query,
       b.trx_id AS blocking_trx, b.trx_mysql_thread_id AS blocking_thread,
       LEFT(b.trx_query, 100) AS blocking_query
FROM performance_schema.data_lock_waits w
JOIN information_schema.innodb_trx b ON b.trx_id = w.blocking_engine_transaction_id
JOIN information_schema.innodb_trx r ON r.trx_id = w.requesting_engine_transaction_id;
```

## Find long-running transactions

Long transactions hold up the undo log and add replica lag.

```sql @tags=diagnose
SELECT trx_id, trx_state, trx_started,
       TIMESTAMPDIFF(SECOND, trx_started, NOW()) AS seconds,
       LEFT(trx_query, 200) AS query
FROM information_schema.innodb_trx
ORDER BY trx_started;
```

## Show the current and maximum connection count

What to look at when you hit Too many connections.

```sql @tags=diagnose
SHOW STATUS LIKE 'Threads_connected';
SHOW VARIABLES LIKE 'max_connections';
```

## Explain a query's plan

```sql @tags=diagnose
EXPLAIN ANALYZE {{sql}};
```

## Check whether the slow query log is on

```sql @tags=diagnose
SHOW VARIABLES LIKE 'slow_query%';
SHOW VARIABLES LIKE 'long_query_time';
```

## Turn the slow query log on temporarily

Reverts on restart.

```sql @tags=diagnose @confirm
SET GLOBAL slow_query_log = 'ON';
SET GLOBAL long_query_time = 1;
```

## Summarise the slow log with mysqldumpslow

```sh @tags=diagnose
mysqldumpslow -s t -t 20 {{file}}
```

## Check index usage

```sql @tags=diagnose
SELECT object_schema, object_name, index_name, count_star
FROM performance_schema.table_io_waits_summary_by_index_usage
WHERE object_schema = DATABASE() AND index_name IS NOT NULL
ORDER BY count_star ASC
LIMIT 20;
```

## Show replication status

`Seconds_Behind_Master` is the lag in seconds.

```sql @tags=replication
SHOW REPLICA STATUS\G
```

## Check whether the character set is utf8mb4

Emoji that will not save and mangled non-ASCII text are almost always this.

```sql @tags=diagnose
SELECT table_name, table_collation
FROM information_schema.tables
WHERE table_schema = DATABASE();
```

## Convert a table to utf8mb4

```sql @confirm
ALTER TABLE {{table}} CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci;
```

## Create a read-only user

```sql @confirm
CREATE USER '{{user}}'@'%' IDENTIFIED BY '{{password}}';
GRANT SELECT ON {{db}}.* TO '{{user}}'@'%';
FLUSH PRIVILEGES;
```

## Show a user's grants

```sql
SHOW GRANTS FOR '{{user}}'@'%';
```

## Run a local MySQL in Docker

```sh @tags=docker
docker run -d --name mysql -e MYSQL_ROOT_PASSWORD={{password}} -e MYSQL_DATABASE={{db}} -p 3306:3306 mysql:8 --character-set-server=utf8mb4
```

## Show the version

```sql
SELECT VERSION();
```
