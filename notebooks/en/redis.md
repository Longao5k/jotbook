---
name: redis
description: Redis - connecting, keys, memory diagnostics, persistence
tags: [db, cache]
vars:
  host:
    desc: host
    from: profile
---

## Connect

```sh @tags=connect
redis-cli -h {{host}} -p {{port=6379}}
```

## Connect with a password

Do not put the password on the command line; it lands in your shell history. Use AUTH once connected.

```sh @tags=connect
redis-cli -h {{host}} -p {{port=6379}} --no-auth-warning
```

## Connect to a specific database

```sh @tags=connect
redis-cli -h {{host}} -n {{index}}
```

## Run one command and exit

```sh @tags=connect
redis-cli -h {{host}} {{command}}
```

## Test the connection

```sh @tags=connect
redis-cli -h {{host}} ping
```

## Scan keys safely

**Never run KEYS in production**; it blocks the whole instance. SCAN is cursor-based and does not.

```sh @tags=keys
redis-cli -h {{host}} --scan --pattern "{{pattern}}"
```

## Count the keys matching a pattern

```sh @tags=keys
redis-cli -h {{host}} --scan --pattern "{{pattern}}" | wc -l
```

## Delete keys matching a pattern

Delete in batches rather than pulling the whole set at once.

```sh @tags=keys @confirm
redis-cli -h {{host}} --scan --pattern "{{pattern}}" | xargs -L 100 redis-cli -h {{host}} UNLINK
```

## Show a key's type and remaining TTL

```sh @tags=keys
redis-cli -h {{host}} -c "TYPE {{key}}" && redis-cli -h {{host}} TTL {{key}}
```

## Delete a key without blocking

DEL blocks on large keys; UNLINK reclaims in the background.

```sh @tags=keys @confirm
redis-cli -h {{host}} UNLINK {{key}}
```

## Find the keys using the most memory

A sampling scan, with very little impact on a live instance.

```sh @tags=diagnose
redis-cli -h {{host}} --bigkeys
```

## Find the most frequently accessed keys

```sh @tags=diagnose
redis-cli -h {{host}} --hotkeys
```

## Show how much memory one key uses

```sh @tags=diagnose
redis-cli -h {{host}} MEMORY USAGE {{key}}
```

## Show overall memory usage

```sh @tags=diagnose
redis-cli -h {{host}} INFO memory
```

## Show connection and client counts

```sh @tags=diagnose
redis-cli -h {{host}} INFO clients
```

## Show the hit rate

`keyspace_hits / (hits + misses)`. Below 0.9 means the caching strategy needs work.

```sh @tags=diagnose
redis-cli -h {{host}} INFO stats | grep keyspace
```

## Watch commands as they execute

Excellent for debugging, but it is noisy, so do not leave it running.

```sh @tags=diagnose @remote
redis-cli -h {{host}} MONITOR
```

## Show the slow log

```sh @tags=diagnose
redis-cli -h {{host}} SLOWLOG GET 20
```

## Clear the slow log

```sh @tags=diagnose
redis-cli -h {{host}} SLOWLOG RESET
```

## Sample latency live

```sh @tags=diagnose
redis-cli -h {{host}} --latency
```

## Show the key count per database

```sh @tags=diagnose
redis-cli -h {{host}} INFO keyspace
```

## Show every configuration value

```sh
redis-cli -h {{host}} CONFIG GET "*"
```

## Show one configuration value

```sh
redis-cli -h {{host}} CONFIG GET {{param}}
```

## Change configuration at runtime

Reverts on restart; use CONFIG REWRITE to persist it.

```sh @confirm
redis-cli -h {{host}} CONFIG SET {{param}} {{value}}
```

## Write the runtime configuration back to the config file

```sh @confirm
redis-cli -h {{host}} CONFIG REWRITE
```

## Trigger a background snapshot

```sh @tags=persist @confirm
redis-cli -h {{host}} BGSAVE
```

## Show when the last snapshot was taken

```sh @tags=persist
redis-cli -h {{host}} LASTSAVE
```

## Rewrite the AOF file

For when the AOF has grown too large.

```sh @tags=persist @confirm
redis-cli -h {{host}} BGREWRITEAOF
```

## Flush the current database

Not reversible.

```sh @confirm
redis-cli -h {{host}} FLUSHDB ASYNC
```

## Flush every database

Not reversible, and it is every database.

```sh @confirm
redis-cli -h {{host}} FLUSHALL ASYNC
```

## Show replication status

```sh @tags=replication
redis-cli -h {{host}} INFO replication
```

## Show cluster status

```sh @tags=cluster
redis-cli -h {{host}} CLUSTER INFO
```

## List cluster nodes

```sh @tags=cluster
redis-cli -h {{host}} CLUSTER NODES
```

## Run a local Redis in Docker

```sh @tags=docker
docker run -d --name redis -p 6379:6379 redis:7-alpine redis-server --appendonly yes
```

## Benchmark

```sh
redis-benchmark -h {{host}} -n 10000 -c 50 -q
```
