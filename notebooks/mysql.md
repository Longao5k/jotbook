---
name: mysql
description: MySQL / MariaDB —— 连接、备份恢复、慢查询与锁诊断
tags: [db, sql]
vars:
  host:
    desc: 主机
    from: profile
  db:
    desc: 数据库名
    from: profile
  user:
    desc: 用户名
    from: profile
---

## 连接

不要在命令行里写 `-p密码`，会进 shell 历史。用 -p 让它交互提示。

```sh @tags=connect
mysql -h {{host}} -u {{user}} -p {{db}}
```

## 执行一条 SQL 后退出

```sh @tags=connect
mysql -h {{host}} -u {{user}} -p -e "{{sql}}" {{db}}
```

## 执行一个 SQL 文件

```sh @tags=connect
mysql -h {{host}} -u {{user}} -p {{db}} < {{file}}
```

## 把查询结果导成 TSV

```sh @tags=connect
mysql -h {{host}} -u {{user}} -p -B -e "{{sql}}" {{db}} > {{file}}.tsv
```

## 客户端元命令速查

进到 mysql 之后用这些。

```txt @tags=reference
SHOW DATABASES;              列出所有数据库
USE dbname;                  切换数据库
SHOW TABLES;                 列出当前库的表
DESC tablename;              查看表结构
SHOW CREATE TABLE t\G        查看建表语句（含索引和引擎）
SHOW INDEX FROM t;           查看索引
SHOW PROCESSLIST;            查看当前连接和正在跑的查询
SHOW ENGINE INNODB STATUS\G  InnoDB 详情（死锁信息在这里）
STATUS;                      连接信息和字符集
\G                           把结果竖排显示（宽表必备）
\q                           退出
```

## 备份单个数据库

`--single-transaction` 让 InnoDB 不锁表就能拿到一致性快照。

```sh @tags=backup
mysqldump -h {{host}} -u {{user}} -p --single-transaction --routines --triggers {{db}} > {{file}}.sql
```

## 备份并压缩

```sh @tags=backup
mysqldump -h {{host}} -u {{user}} -p --single-transaction {{db}} | gzip > {{file}}.sql.gz
```

## 只备份表结构

```sh @tags=backup
mysqldump -h {{host}} -u {{user}} -p --no-data {{db}} > {{file}}.sql
```

## 只备份指定的表

```sh @tags=backup
mysqldump -h {{host}} -u {{user}} -p --single-transaction {{db}} {{table}} > {{file}}.sql
```

## 恢复

```sh @tags=backup @confirm
mysql -h {{host}} -u {{user}} -p {{db}} < {{file}}.sql
```

## 从压缩包恢复

```sh @tags=backup @confirm
gunzip < {{file}}.sql.gz | mysql -h {{host}} -u {{user}} -p {{db}}
```

## 查看每张表占多少空间

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

## 查看各数据库总大小

```sql @tags=diagnose
SELECT table_schema AS db,
       ROUND(SUM(data_length + index_length) / 1024 / 1024, 1) AS size_mb
FROM information_schema.tables
GROUP BY table_schema
ORDER BY size_mb DESC;
```

## 查看当前正在执行的查询

```sql @tags=diagnose
SELECT id, user, host, db, command, time, state, LEFT(info, 200) AS query
FROM information_schema.processlist
WHERE command <> 'Sleep'
ORDER BY time DESC;
```

## 结束一个连接

```sql @tags=diagnose @confirm
KILL {{id}};
```

## 查看锁等待

```sql @tags=diagnose
SELECT r.trx_id AS waiting_trx, r.trx_mysql_thread_id AS waiting_thread,
       LEFT(r.trx_query, 100) AS waiting_query,
       b.trx_id AS blocking_trx, b.trx_mysql_thread_id AS blocking_thread,
       LEFT(b.trx_query, 100) AS blocking_query
FROM performance_schema.data_lock_waits w
JOIN information_schema.innodb_trx b ON b.trx_id = w.blocking_engine_transaction_id
JOIN information_schema.innodb_trx r ON r.trx_id = w.requesting_engine_transaction_id;
```

## 查看跑得久的事务

长事务会拖住 undo log 和主从延迟。

```sql @tags=diagnose
SELECT trx_id, trx_state, trx_started,
       TIMESTAMPDIFF(SECOND, trx_started, NOW()) AS seconds,
       LEFT(trx_query, 200) AS query
FROM information_schema.innodb_trx
ORDER BY trx_started;
```

## 查看连接数和上限

报 Too many connections 时看这个。

```sql @tags=diagnose
SHOW STATUS LIKE 'Threads_connected';
SHOW VARIABLES LIKE 'max_connections';
```

## 分析一条查询的执行计划

```sql @tags=diagnose
EXPLAIN ANALYZE {{sql}};
```

## 查看慢查询日志有没有开

```sql @tags=diagnose
SHOW VARIABLES LIKE 'slow_query%';
SHOW VARIABLES LIKE 'long_query_time';
```

## 临时打开慢查询日志

重启后失效。

```sql @tags=diagnose @confirm
SET GLOBAL slow_query_log = 'ON';
SET GLOBAL long_query_time = 1;
```

## 用 mysqldumpslow 汇总慢查询

```sh @tags=diagnose
mysqldumpslow -s t -t 20 {{file}}
```

## 查看索引使用情况

```sql @tags=diagnose
SELECT object_schema, object_name, index_name, count_star
FROM performance_schema.table_io_waits_summary_by_index_usage
WHERE object_schema = DATABASE() AND index_name IS NOT NULL
ORDER BY count_star ASC
LIMIT 20;
```

## 查看主从复制状态

`Seconds_Behind_Master` 是延迟秒数。

```sql @tags=replication
SHOW REPLICA STATUS\G
```

## 查看字符集是不是 utf8mb4

emoji 存不进去、中文乱码，八成是这里不对。

```sql @tags=diagnose
SELECT table_name, table_collation
FROM information_schema.tables
WHERE table_schema = DATABASE();
```

## 把一张表转成 utf8mb4

```sql @confirm
ALTER TABLE {{table}} CONVERT TO CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci;
```

## 创建只读用户

```sql @confirm
CREATE USER '{{user}}'@'%' IDENTIFIED BY '{{password}}';
GRANT SELECT ON {{db}}.* TO '{{user}}'@'%';
FLUSH PRIVILEGES;
```

## 查看某个用户的权限

```sql
SHOW GRANTS FOR '{{user}}'@'%';
```

## 用 Docker 起一个本地 MySQL

```sh @tags=docker
docker run -d --name mysql -e MYSQL_ROOT_PASSWORD={{password}} -e MYSQL_DATABASE={{db}} -p 3306:3306 mysql:8 --character-set-server=utf8mb4
```

## 查看版本

```sql
SELECT VERSION();
```
