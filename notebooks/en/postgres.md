---
name: postgres
description: PostgreSQL - psql meta-commands, backup and restore, slow query and index diagnostics
tags: [db, sql]
vars:
  db:
    desc: database
    from: profile
  host:
    desc: host
    from: profile
  user:
    desc: user
    from: profile
---

## Connect

```sh @tags=connect
psql -h {{host}} -U {{user}} -d {{db}}
```

## Connect with a connection string

```sh @tags=connect
psql "postgresql://{{user}}@{{host}}:5432/{{db}}"
```

## Run one statement and exit

For scripts.

```sh @tags=connect
psql -h {{host}} -U {{user}} -d {{db}} -c "{{sql}}"
```

## Run a SQL file

```sh @tags=connect
psql -h {{host}} -U {{user}} -d {{db}} -f {{file}}
```

## Export a query result as CSV

```sh @tags=connect
psql -h {{host}} -U {{user}} -d {{db}} -c "\copy ({{sql}}) TO '{{file}}' CSV HEADER"
```

## psql meta-command reference

Use these once inside psql. This is the pile nobody ever remembers.

```txt @tags=reference
\l              list databases
\c dbname       switch database
\dt             list tables in the current schema
\dt *.*         list tables in every schema
\d table        show columns, indexes and constraints
\d+ table       the same, plus size and comments
\di             list indexes
\dv             list views
\df             list functions
\du             list roles and privileges
\dn             list schemas
\x              toggle expanded output (essential for wide tables)
\timing         toggle query timing
\e              write this statement in an editor
\i file.sql     run a SQL file
\copy ... TO    export to a local file
\q              quit
```

## Back up a database in the custom format

The custom format (-F c) restores in parallel and can restore selected tables, which makes it far more useful than plain SQL.

```sh @tags=backup
pg_dump -h {{host}} -U {{user}} -d {{db}} -F c -f {{file}}.dump
```

## Back up the schema only

```sh @tags=backup
pg_dump -h {{host}} -U {{user}} -d {{db}} -s -f {{file}}.sql
```

## Back up the data only

```sh @tags=backup
pg_dump -h {{host}} -U {{user}} -d {{db}} -a -f {{file}}.sql
```

## Back up specific tables

```sh @tags=backup
pg_dump -h {{host}} -U {{user}} -d {{db}} -t {{table}} -F c -f {{file}}.dump
```

## Restore from a dump

```sh @tags=backup @confirm
pg_restore -h {{host}} -U {{user}} -d {{db}} --clean --if-exists {{file}}.dump
```

## Restore in parallel, much faster

```sh @tags=backup @confirm
pg_restore -h {{host}} -U {{user}} -d {{db}} -j 4 {{file}}.dump
```

## See how much space each table uses

The first diagnostic to run once a database starts growing.

```sql @tags=diagnose
SELECT relname AS table_name,
       pg_size_pretty(pg_total_relation_size(relid)) AS total_size,
       pg_size_pretty(pg_relation_size(relid))       AS table_size,
       pg_size_pretty(pg_indexes_size(relid))        AS index_size
FROM pg_catalog.pg_statio_user_tables
ORDER BY pg_total_relation_size(relid) DESC
LIMIT 20;
```

## See the total size of each database

```sql @tags=diagnose
SELECT datname, pg_size_pretty(pg_database_size(datname)) AS size
FROM pg_database
ORDER BY pg_database_size(datname) DESC;
```

## Show currently running queries

What to look at when production stalls.

```sql @tags=diagnose
SELECT pid, now() - query_start AS duration, state, left(query, 120) AS query
FROM pg_stat_activity
WHERE state <> 'idle' AND pid <> pg_backend_pid()
ORDER BY duration DESC;
```

## Find queries running longer than five minutes

```sql @tags=diagnose
SELECT pid, now() - query_start AS duration, left(query, 200) AS query
FROM pg_stat_activity
WHERE state = 'active' AND now() - query_start > interval '5 minutes'
ORDER BY duration DESC;
```

## Cancel a query gracefully

Try this first and let the query end itself.

```sql @tags=diagnose @confirm
SELECT pg_cancel_backend({{pid}});
```

## Terminate a connection

Only when cancel does nothing; this kills the session outright.

```sql @tags=diagnose @confirm
SELECT pg_terminate_backend({{pid}});
```

## Show lock waits

For when one transaction is blocking everyone else.

```sql @tags=diagnose
SELECT blocked.pid AS blocked_pid, blocked.query AS blocked_query,
       blocking.pid AS blocking_pid, blocking.query AS blocking_query
FROM pg_stat_activity blocked
JOIN pg_stat_activity blocking ON blocking.pid = ANY(pg_blocking_pids(blocked.pid))
WHERE cardinality(pg_blocking_pids(blocked.pid)) > 0;
```

## Find indexes that have never been used

They cost space and slow writes down, so consider dropping them.

```sql @tags=diagnose
SELECT schemaname, relname AS table_name, indexrelname AS index_name,
       idx_scan, pg_size_pretty(pg_relation_size(indexrelid)) AS size
FROM pg_stat_user_indexes
WHERE idx_scan = 0
ORDER BY pg_relation_size(indexrelid) DESC;
```

## Find the tables with the most sequential scans

Usually a sign of a missing index.

```sql @tags=diagnose
SELECT relname AS table_name, seq_scan, idx_scan, n_live_tup
FROM pg_stat_user_tables
WHERE seq_scan > 0
ORDER BY seq_scan DESC
LIMIT 20;
```

## Show the current and maximum connection count

For when you hit too many connections.

```sql @tags=diagnose
SELECT count(*) AS current, setting AS max
FROM pg_stat_activity, pg_settings
WHERE pg_settings.name = 'max_connections'
GROUP BY setting;
```

## Count connections per database

```sql @tags=diagnose
SELECT datname, count(*) FROM pg_stat_activity GROUP BY datname ORDER BY count DESC;
```

## Check table bloat

Dead tuples pile up when autovacuum cannot keep up.

```sql @tags=diagnose
SELECT relname, n_live_tup, n_dead_tup,
       round(n_dead_tup * 100.0 / NULLIF(n_live_tup + n_dead_tup, 0), 1) AS dead_pct,
       last_autovacuum
FROM pg_stat_user_tables
WHERE n_dead_tup > 1000
ORDER BY dead_pct DESC NULLS LAST;
```

## Explain a query's plan

ANALYZE really executes it, so wrap writes in a transaction and roll back.

```sql @tags=diagnose
EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT) {{sql}};
```

## Create an index without locking the table

Adding an index in production must use CONCURRENTLY, or writes to the whole table block.

```sql @confirm
CREATE INDEX CONCURRENTLY idx_{{table}}_{{column}} ON {{table}} ({{column}});
```

## Drop an index without locking the table

```sql @confirm
DROP INDEX CONCURRENTLY IF EXISTS {{index}};
```

## Reclaim space in a table by hand

VACUUM FULL holds an exclusive lock throughout, so it belongs in a maintenance window.

```sql @confirm
VACUUM (VERBOSE, ANALYZE) {{table}};
```

## Refresh the statistics

The first thing to try when a query plan degrades.

```sql
ANALYZE {{table}};
```

## Show every index definition on a table

```sql
SELECT indexname, indexdef FROM pg_indexes WHERE tablename = '{{table}}';
```

## Rename a table safely, inside a transaction

```sql @confirm
BEGIN;
ALTER TABLE {{old}} RENAME TO {{new}};
-- COMMIT once it looks right; ROLLBACK if not
COMMIT;
```

## Grant a user read-only access

```sql @confirm
GRANT CONNECT ON DATABASE {{db}} TO {{user}};
GRANT USAGE ON SCHEMA public TO {{user}};
GRANT SELECT ON ALL TABLES IN SCHEMA public TO {{user}};
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO {{user}};
```

## Show the current user and database

```sql
SELECT current_user, current_database(), version();
```
