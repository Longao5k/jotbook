---
name: postgres
description: PostgreSQL —— psql 元命令、备份恢复、慢查询与索引诊断
tags: [db, sql]
vars:
  db:
    desc: 数据库名
    from: profile
  host:
    desc: 主机
    from: profile
  user:
    desc: 用户名
    from: profile
---

## 连接数据库

```sh @tags=connect
psql -h {{host}} -U {{user}} -d {{db}}
```

## 用连接串连接

```sh @tags=connect
psql "postgresql://{{user}}@{{host}}:5432/{{db}}"
```

## 执行单条 SQL 后退出

写脚本时用。

```sh @tags=connect
psql -h {{host}} -U {{user}} -d {{db}} -c "{{sql}}"
```

## 执行一个 SQL 文件

```sh @tags=connect
psql -h {{host}} -U {{user}} -d {{db}} -f {{file}}
```

## 导出查询结果为 CSV

```sh @tags=connect
psql -h {{host}} -U {{user}} -d {{db}} -c "\copy ({{sql}}) TO '{{file}}' CSV HEADER"
```

## psql 元命令速查

进到 psql 里之后用这些，永远记不住的就是这一堆。

```txt @tags=reference
\l              列出所有数据库
\c dbname       切换数据库
\dt             列出当前 schema 的表
\dt *.*         列出所有 schema 的表
\d table        查看表结构、索引、约束
\d+ table       同上，外加体积和注释
\di             列出索引
\dv             列出视图
\df             列出函数
\du             列出角色和权限
\dn             列出 schema
\x              开关竖排显示（宽表必备）
\timing         开关执行耗时显示
\e              用编辑器写这条 SQL
\i file.sql     执行 SQL 文件
\copy ... TO    导出到本地文件
\q              退出
```

## 备份整个数据库为自定义格式

自定义格式（-F c）可以并行恢复、可以只恢复部分表，比纯 SQL 好用。

```sh @tags=backup
pg_dump -h {{host}} -U {{user}} -d {{db}} -F c -f {{file}}.dump
```

## 只备份表结构

```sh @tags=backup
pg_dump -h {{host}} -U {{user}} -d {{db}} -s -f {{file}}.sql
```

## 只备份数据

```sh @tags=backup
pg_dump -h {{host}} -U {{user}} -d {{db}} -a -f {{file}}.sql
```

## 只备份指定的几张表

```sh @tags=backup
pg_dump -h {{host}} -U {{user}} -d {{db}} -t {{table}} -F c -f {{file}}.dump
```

## 从 dump 恢复

```sh @tags=backup @confirm
pg_restore -h {{host}} -U {{user}} -d {{db}} --clean --if-exists {{file}}.dump
```

## 并行恢复，快很多

```sh @tags=backup @confirm
pg_restore -h {{host}} -U {{user}} -d {{db}} -j 4 {{file}}.dump
```

## 查看每张表占了多少空间

数据库变大时第一条要跑的诊断。

```sql @tags=diagnose
SELECT relname AS table_name,
       pg_size_pretty(pg_total_relation_size(relid)) AS total_size,
       pg_size_pretty(pg_relation_size(relid))       AS table_size,
       pg_size_pretty(pg_indexes_size(relid))        AS index_size
FROM pg_catalog.pg_statio_user_tables
ORDER BY pg_total_relation_size(relid) DESC
LIMIT 20;
```

## 查看数据库总体积

```sql @tags=diagnose
SELECT datname, pg_size_pretty(pg_database_size(datname)) AS size
FROM pg_database
ORDER BY pg_database_size(datname) DESC;
```

## 查看当前正在执行的查询

线上卡住时看这个。

```sql @tags=diagnose
SELECT pid, now() - query_start AS duration, state, left(query, 120) AS query
FROM pg_stat_activity
WHERE state <> 'idle' AND pid <> pg_backend_pid()
ORDER BY duration DESC;
```

## 找出跑了超过 5 分钟的查询

```sql @tags=diagnose
SELECT pid, now() - query_start AS duration, left(query, 200) AS query
FROM pg_stat_activity
WHERE state = 'active' AND now() - query_start > interval '5 minutes'
ORDER BY duration DESC;
```

## 温和地取消一个查询

先试这个，让查询自己结束。

```sql @tags=diagnose @confirm
SELECT pg_cancel_backend({{pid}});
```

## 强制断开一个连接

cancel 无效时才用，会直接终止会话。

```sql @tags=diagnose @confirm
SELECT pg_terminate_backend({{pid}});
```

## 查看锁等待

一个事务卡住其他所有人时用。

```sql @tags=diagnose
SELECT blocked.pid AS blocked_pid, blocked.query AS blocked_query,
       blocking.pid AS blocking_pid, blocking.query AS blocking_query
FROM pg_stat_activity blocked
JOIN pg_stat_activity blocking ON blocking.pid = ANY(pg_blocking_pids(blocked.pid))
WHERE cardinality(pg_blocking_pids(blocked.pid)) > 0;
```

## 找出从来没被用过的索引

白占空间还拖慢写入，可以考虑删掉。

```sql @tags=diagnose
SELECT schemaname, relname AS table_name, indexrelname AS index_name,
       idx_scan, pg_size_pretty(pg_relation_size(indexrelid)) AS size
FROM pg_stat_user_indexes
WHERE idx_scan = 0
ORDER BY pg_relation_size(indexrelid) DESC;
```

## 找出全表扫描次数最多的表

通常意味着缺索引。

```sql @tags=diagnose
SELECT relname AS table_name, seq_scan, idx_scan, n_live_tup
FROM pg_stat_user_tables
WHERE seq_scan > 0
ORDER BY seq_scan DESC
LIMIT 20;
```

## 查看当前连接数与上限

报 too many connections 时用。

```sql @tags=diagnose
SELECT count(*) AS current, setting AS max
FROM pg_stat_activity, pg_settings
WHERE pg_settings.name = 'max_connections'
GROUP BY setting;
```

## 按数据库统计连接数

```sql @tags=diagnose
SELECT datname, count(*) FROM pg_stat_activity GROUP BY datname ORDER BY count DESC;
```

## 查看表膨胀情况

autovacuum 跟不上时死元组会堆积。

```sql @tags=diagnose
SELECT relname, n_live_tup, n_dead_tup,
       round(n_dead_tup * 100.0 / NULLIF(n_live_tup + n_dead_tup, 0), 1) AS dead_pct,
       last_autovacuum
FROM pg_stat_user_tables
WHERE n_dead_tup > 1000
ORDER BY dead_pct DESC NULLS LAST;
```

## 分析一条查询的执行计划

加 ANALYZE 会真的执行，写操作要放在事务里回滚。

```sql @tags=diagnose
EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT) {{sql}};
```

## 不锁表地创建索引

生产环境加索引必须加 CONCURRENTLY，否则会阻塞整张表的写入。

```sql @confirm
CREATE INDEX CONCURRENTLY idx_{{table}}_{{column}} ON {{table}} ({{column}});
```

## 不锁表地删除索引

```sql @confirm
DROP INDEX CONCURRENTLY IF EXISTS {{index}};
```

## 手动回收一张表的空间

VACUUM FULL 会全程锁表，只能在维护窗口做。

```sql @confirm
VACUUM (VERBOSE, ANALYZE) {{table}};
```

## 更新统计信息

执行计划变差时先试这个。

```sql
ANALYZE {{table}};
```

## 查看某张表的所有索引定义

```sql
SELECT indexname, indexdef FROM pg_indexes WHERE tablename = '{{table}}';
```

## 安全地重命名表（在事务里）

```sql @confirm
BEGIN;
ALTER TABLE {{old}} RENAME TO {{new}};
-- 确认无误后 COMMIT; 出错就 ROLLBACK;
COMMIT;
```

## 给用户授予只读权限

```sql @confirm
GRANT CONNECT ON DATABASE {{db}} TO {{user}};
GRANT USAGE ON SCHEMA public TO {{user}};
GRANT SELECT ON ALL TABLES IN SCHEMA public TO {{user}};
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO {{user}};
```

## 查看当前用户和数据库

```sql
SELECT current_user, current_database(), version();
```
