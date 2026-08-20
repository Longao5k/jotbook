---
name: mssql
description: SQL Server - sqlcmd, backup and restore, blocking and index diagnostics
tags: [db, sql, dotnet]
vars:
  server:
    desc: server
    from: profile
  db:
    desc: database
    from: profile
  user:
    desc: login
    from: profile
---

## Connect with SQL authentication

```sh @tags=connect
sqlcmd -S {{server}} -U {{user}} -d {{db}} -C
```

## Connect with Windows authentication

```sh @tags=connect @platform=windows
sqlcmd -S {{server}} -E -d {{db}} -C
```

## Run one statement and exit

```sh @tags=connect
sqlcmd -S {{server}} -U {{user}} -d {{db}} -C -Q "{{sql}}"
```

## Run a SQL file

```sh @tags=connect
sqlcmd -S {{server}} -U {{user}} -d {{db}} -C -i {{file}}
```

## Export a query result as CSV

```sh @tags=connect
sqlcmd -S {{server}} -U {{user}} -d {{db}} -C -Q "{{sql}}" -s "," -W -h -1 -o {{file}}.csv
```

## Run a local SQL Server in Docker

By far the easiest way to develop locally, with nothing installed.

```sh @tags=docker
docker run -d --name mssql -e "ACCEPT_EULA=Y" -e "MSSQL_SA_PASSWORD={{password}}" -p 1433:1433 mcr.microsoft.com/mssql/server:2022-latest
```

## Back up a database

The path is a path **on the server**, not on your machine.

```sql @tags=backup @confirm
BACKUP DATABASE [{{db}}]
TO DISK = N'{{path}}'
WITH FORMAT, INIT, COMPRESSION, STATS = 10;
```

## Restore from a backup

```sql @tags=backup @confirm
RESTORE DATABASE [{{db}}]
FROM DISK = N'{{path}}'
WITH REPLACE, RECOVERY, STATS = 10;
```

## List the logical files inside a backup

You need the logical names before restoring to different paths.

```sql @tags=backup
RESTORE FILELISTONLY FROM DISK = N'{{path}}';
```

## See how much space each table uses

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

## See data file sizes and free space

```sql @tags=diagnose
SELECT name AS logical_name, type_desc, physical_name,
       CAST(size * 8.0 / 1024 AS DECIMAL(10,1)) AS size_mb,
       CAST(FILEPROPERTY(name, 'SpaceUsed') * 8.0 / 1024 AS DECIMAL(10,1)) AS used_mb
FROM sys.database_files;
```

## Show currently executing requests

The first thing to run when production stalls.

```sql @tags=diagnose
SELECT r.session_id, r.status, r.wait_type, r.wait_time,
       r.blocking_session_id, DB_NAME(r.database_id) AS db,
       SUBSTRING(t.text, 1, 200) AS query
FROM sys.dm_exec_requests r
CROSS APPLY sys.dm_exec_sql_text(r.sql_handle) t
WHERE r.session_id <> @@SPID
ORDER BY r.total_elapsed_time DESC;
```

## Find out who is blocking whom

```sql @tags=diagnose
SELECT blocked.session_id  AS blocked_spid,
       blocked.blocking_session_id AS blocker_spid,
       blocked.wait_type, blocked.wait_time,
       SUBSTRING(t.text, 1, 200) AS blocked_query
FROM sys.dm_exec_requests blocked
CROSS APPLY sys.dm_exec_sql_text(blocked.sql_handle) t
WHERE blocked.blocking_session_id <> 0;
```

## Kill a session

```sql @tags=diagnose @confirm
KILL {{spid}};
```

## Find the most expensive queries

Where tuning starts.

```sql @tags=diagnose
SELECT TOP 20
       qs.total_elapsed_time / qs.execution_count / 1000 AS avg_ms,
       qs.execution_count,
       SUBSTRING(t.text, (qs.statement_start_offset/2) + 1, 200) AS query
FROM sys.dm_exec_query_stats qs
CROSS APPLY sys.dm_exec_sql_text(qs.sql_handle) t
ORDER BY avg_ms DESC;
```

## Show missing index suggestions

SQL Server works these out itself. Worth reading, but do not apply them blindly.

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

## Find indexes that have never been used

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

## Check index fragmentation

Only worth rebuilding above about 30%.

```sql @tags=diagnose
SELECT OBJECT_NAME(ips.object_id) AS table_name, i.name AS index_name,
       CAST(ips.avg_fragmentation_in_percent AS DECIMAL(5,1)) AS frag_pct,
       ips.page_count
FROM sys.dm_db_index_physical_stats(DB_ID(), NULL, NULL, NULL, 'LIMITED') ips
JOIN sys.indexes i ON ips.object_id = i.object_id AND ips.index_id = i.index_id
WHERE ips.page_count > 1000 AND ips.avg_fragmentation_in_percent > 10
ORDER BY frag_pct DESC;
```

## Rebuild an index online

ONLINE needs Enterprise; on Standard drop that clause, but it will lock the table.

```sql @confirm
ALTER INDEX {{index}} ON {{table}} REBUILD WITH (ONLINE = ON, MAXDOP = 4);
```

## Update statistics

The first thing to try when a plan suddenly degrades.

```sql
UPDATE STATISTICS {{table}} WITH FULLSCAN;
```

## Show a table's structure

```sql
SELECT c.name AS column_name, t.name AS type,
       c.max_length, c.precision, c.scale, c.is_nullable
FROM sys.columns c
JOIN sys.types t ON c.user_type_id = t.user_type_id
WHERE c.object_id = OBJECT_ID('{{table}}')
ORDER BY c.column_id;
```

## List every table with its row count

```sql
SELECT t.name AS table_name, SUM(p.rows) AS row_count
FROM sys.tables t
JOIN sys.partitions p ON t.object_id = p.object_id
WHERE p.index_id IN (0, 1)
GROUP BY t.name
ORDER BY row_count DESC;
```

## Search the text of every stored procedure

Check who references a column before renaming it.

```sql
SELECT OBJECT_NAME(object_id) AS name
FROM sys.sql_modules
WHERE definition LIKE '%{{keyword}}%';
```

## Put the database into single-user mode

For restores and schema changes; it kicks out every connection.

```sql @confirm
ALTER DATABASE [{{db}}] SET SINGLE_USER WITH ROLLBACK IMMEDIATE;
```

## Back to multi-user mode

```sql @confirm
ALTER DATABASE [{{db}}] SET MULTI_USER;
```

## Show the version and edition

```sql
SELECT @@VERSION AS version, SERVERPROPERTY('Edition') AS edition;
```

## Create a read-only login

```sql @confirm
CREATE LOGIN [{{user}}] WITH PASSWORD = '{{password}}';
USE [{{db}}];
CREATE USER [{{user}}] FOR LOGIN [{{user}}];
ALTER ROLE db_datareader ADD MEMBER [{{user}}];
```

## See what sessions are waiting on

```sql @tags=diagnose
SELECT wait_type, waiting_tasks_count, wait_time_ms, signal_wait_time_ms
FROM sys.dm_os_wait_stats
WHERE wait_type NOT LIKE '%SLEEP%' AND wait_time_ms > 0
ORDER BY wait_time_ms DESC;
```
