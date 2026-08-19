---
name: mssql
description: SQL Server —— sqlcmd、备份恢复、阻塞与索引诊断
tags: [db, sql, dotnet]
vars:
  server:
    desc: 服务器
    from: profile
  db:
    desc: 数据库名
    from: profile
  user:
    desc: 登录名
    from: profile
---

## 用 SQL 登录连接

```sh @tags=connect
sqlcmd -S {{server}} -U {{user}} -d {{db}} -C
```

## 用 Windows 集成认证连接

```sh @tags=connect @platform=windows
sqlcmd -S {{server}} -E -d {{db}} -C
```

## 执行一条 SQL 后退出

```sh @tags=connect
sqlcmd -S {{server}} -U {{user}} -d {{db}} -C -Q "{{sql}}"
```

## 执行一个 SQL 文件

```sh @tags=connect
sqlcmd -S {{server}} -U {{user}} -d {{db}} -C -i {{file}}
```

## 把查询结果导成 CSV

```sh @tags=connect
sqlcmd -S {{server}} -U {{user}} -d {{db}} -C -Q "{{sql}}" -s "," -W -h -1 -o {{file}}.csv
```

## 用 Docker 起一个本地 SQL Server

本地开发最省事的办法，不用装。

```sh @tags=docker
docker run -d --name mssql -e "ACCEPT_EULA=Y" -e "MSSQL_SA_PASSWORD={{password}}" -p 1433:1433 mcr.microsoft.com/mssql/server:2022-latest
```

## 备份数据库

路径是**服务器上**的路径，不是你本机的。

```sql @tags=backup @confirm
BACKUP DATABASE [{{db}}]
TO DISK = N'{{path}}'
WITH FORMAT, INIT, COMPRESSION, STATS = 10;
```

## 从备份恢复

```sql @tags=backup @confirm
RESTORE DATABASE [{{db}}]
FROM DISK = N'{{path}}'
WITH REPLACE, RECOVERY, STATS = 10;
```

## 查看备份文件里有哪些逻辑文件

恢复到不同路径时需要先知道逻辑名。

```sql @tags=backup
RESTORE FILELISTONLY FROM DISK = N'{{path}}';
```

## 查看每张表占多少空间

```sql @tags=diagnose
SELECT t.name AS table_name,
       p.rows AS row_count,
       CAST(SUM(a.total_pages) * 8 / 1024.0 AS DECIMAL(10,1)) AS total_mb,
       CAST(SUM(a.used_pages)  * 8 / 1024.0 AS DECIMAL(10,1)) AS used_mb
FROM sys.tables t
JOIN sys.indexes i      ON t.object_id = i.object_id
JOIN sys.partitions p   ON i.object_id = p.object_id AND i.index_id = p.index_id
JOIN sys.allocation_units a ON p.partition_id = a.container_id
WHERE i.index_id <= 1
GROUP BY t.name, p.rows
ORDER BY total_mb DESC;
```

## 查看数据库文件大小和剩余空间

```sql @tags=diagnose
SELECT name AS logical_name, type_desc, physical_name,
       CAST(size * 8.0 / 1024 AS DECIMAL(10,1)) AS size_mb,
       CAST(FILEPROPERTY(name, 'SpaceUsed') * 8.0 / 1024 AS DECIMAL(10,1)) AS used_mb
FROM sys.database_files;
```

## 查看当前正在执行的请求

线上卡住时第一条。

```sql @tags=diagnose
SELECT r.session_id, r.status, r.wait_type, r.wait_time,
       r.blocking_session_id, DB_NAME(r.database_id) AS db,
       SUBSTRING(t.text, 1, 200) AS query
FROM sys.dm_exec_requests r
CROSS APPLY sys.dm_exec_sql_text(r.sql_handle) t
WHERE r.session_id <> @@SPID
ORDER BY r.total_elapsed_time DESC;
```

## 找出谁阻塞了谁

```sql @tags=diagnose
SELECT blocked.session_id  AS blocked_spid,
       blocked.blocking_session_id AS blocker_spid,
       blocked.wait_type, blocked.wait_time,
       SUBSTRING(t.text, 1, 200) AS blocked_query
FROM sys.dm_exec_requests blocked
CROSS APPLY sys.dm_exec_sql_text(blocked.sql_handle) t
WHERE blocked.blocking_session_id <> 0;
```

## 结束一个会话

```sql @tags=diagnose @confirm
KILL {{spid}};
```

## 查看最耗时的查询

调优起点。

```sql @tags=diagnose
SELECT TOP 20
       qs.total_elapsed_time / qs.execution_count / 1000 AS avg_ms,
       qs.execution_count,
       SUBSTRING(t.text, (qs.statement_start_offset/2) + 1, 200) AS query
FROM sys.dm_exec_query_stats qs
CROSS APPLY sys.dm_exec_sql_text(qs.sql_handle) t
ORDER BY avg_ms DESC;
```

## 查看缺失索引建议

SQL Server 自己算出来的，参考价值很高但别无脑照抄。

```sql @tags=diagnose
SELECT TOP 20
       CAST(s.avg_total_user_cost * s.avg_user_impact * (s.user_seeks + s.user_scans) AS INT) AS score,
       d.statement AS table_name,
       d.equality_columns, d.inequality_columns, d.included_columns
FROM sys.dm_db_missing_index_groups g
JOIN sys.dm_db_missing_index_group_stats s ON g.index_group_handle = s.group_handle
JOIN sys.dm_db_missing_index_details d     ON g.index_handle = d.index_handle
ORDER BY score DESC;
```

## 查看从来没被用过的索引

```sql @tags=diagnose
SELECT OBJECT_NAME(i.object_id) AS table_name, i.name AS index_name,
       s.user_seeks, s.user_scans, s.user_lookups, s.user_updates
FROM sys.indexes i
LEFT JOIN sys.dm_db_index_usage_stats s
       ON i.object_id = s.object_id AND i.index_id = s.index_id
WHERE i.type_desc = 'NONCLUSTERED'
  AND ISNULL(s.user_seeks, 0) + ISNULL(s.user_scans, 0) + ISNULL(s.user_lookups, 0) = 0
ORDER BY s.user_updates DESC;
```

## 查看索引碎片

超过 30% 才值得重建。

```sql @tags=diagnose
SELECT OBJECT_NAME(ips.object_id) AS table_name, i.name AS index_name,
       CAST(ips.avg_fragmentation_in_percent AS DECIMAL(5,1)) AS frag_pct,
       ips.page_count
FROM sys.dm_db_index_physical_stats(DB_ID(), NULL, NULL, NULL, 'LIMITED') ips
JOIN sys.indexes i ON ips.object_id = i.object_id AND ips.index_id = i.index_id
WHERE ips.page_count > 1000 AND ips.avg_fragmentation_in_percent > 10
ORDER BY frag_pct DESC;
```

## 在线重建索引

企业版才支持 ONLINE，标准版去掉那一句但会锁表。

```sql @confirm
ALTER INDEX {{index}} ON {{table}} REBUILD WITH (ONLINE = ON, MAXDOP = 4);
```

## 更新统计信息

执行计划突然变差时先试这个。

```sql
UPDATE STATISTICS {{table}} WITH FULLSCAN;
```

## 查看表结构

```sql
SELECT c.name AS column_name, t.name AS type,
       c.max_length, c.precision, c.scale, c.is_nullable
FROM sys.columns c
JOIN sys.types t ON c.user_type_id = t.user_type_id
WHERE c.object_id = OBJECT_ID('{{table}}')
ORDER BY c.column_id;
```

## 列出所有表和行数

```sql
SELECT t.name AS table_name, SUM(p.rows) AS row_count
FROM sys.tables t
JOIN sys.partitions p ON t.object_id = p.object_id
WHERE p.index_id IN (0, 1)
GROUP BY t.name
ORDER BY row_count DESC;
```

## 搜索所有存储过程的内容

改字段名前先看谁引用了它。

```sql
SELECT OBJECT_NAME(object_id) AS name
FROM sys.sql_modules
WHERE definition LIKE '%{{keyword}}%';
```

## 把数据库切成单用户模式

恢复或改架构前用，会踢掉所有连接。

```sql @confirm
ALTER DATABASE [{{db}}] SET SINGLE_USER WITH ROLLBACK IMMEDIATE;
```

## 切回多用户模式

```sql @confirm
ALTER DATABASE [{{db}}] SET MULTI_USER;
```

## 查看版本和版次

```sql
SELECT @@VERSION AS version, SERVERPROPERTY('Edition') AS edition;
```

## 建一个只读登录

```sql @confirm
CREATE LOGIN [{{user}}] WITH PASSWORD = '{{password}}';
USE [{{db}}];
CREATE USER [{{user}}] FOR LOGIN [{{user}}];
ALTER ROLE db_datareader ADD MEMBER [{{user}}];
```

## 查看某个会话在等什么

```sql @tags=diagnose
SELECT wait_type, waiting_tasks_count, wait_time_ms, signal_wait_time_ms
FROM sys.dm_os_wait_stats
WHERE wait_type NOT LIKE '%SLEEP%' AND wait_time_ms > 0
ORDER BY wait_time_ms DESC;
```
