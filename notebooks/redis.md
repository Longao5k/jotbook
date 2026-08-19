---
name: redis
description: Redis —— 连接、键操作、内存诊断、持久化
tags: [db, cache]
vars:
  host:
    desc: 主机
    from: profile
---

## 连接

```sh @tags=connect
redis-cli -h {{host}} -p {{port=6379}}
```

## 带密码连接

不要在命令行里明文写密码，会进 shell 历史。连上之后用 AUTH。

```sh @tags=connect
redis-cli -h {{host}} -p {{port=6379}} --no-auth-warning
```

## 连接指定数据库

```sh @tags=connect
redis-cli -h {{host}} -n {{index}}
```

## 执行一条命令后退出

```sh @tags=connect
redis-cli -h {{host}} {{command}}
```

## 测试连通

```sh @tags=connect
redis-cli -h {{host}} ping
```

## 安全地扫描键

**永远不要在生产上用 KEYS**，它会阻塞整个实例。SCAN 是游标式的，不阻塞。

```sh @tags=keys
redis-cli -h {{host}} --scan --pattern "{{pattern}}"
```

## 统计匹配某个模式的键有多少

```sh @tags=keys
redis-cli -h {{host}} --scan --pattern "{{pattern}}" | wc -l
```

## 批量删除匹配的键

分批删，别一次性拉全量。

```sh @tags=keys @confirm
redis-cli -h {{host}} --scan --pattern "{{pattern}}" | xargs -L 100 redis-cli -h {{host}} UNLINK
```

## 查看键的类型和剩余存活时间

```sh @tags=keys
redis-cli -h {{host}} -c "TYPE {{key}}" && redis-cli -h {{host}} TTL {{key}}
```

## 非阻塞删除一个键

DEL 在删大 key 时会阻塞，UNLINK 是后台回收。

```sh @tags=keys @confirm
redis-cli -h {{host}} UNLINK {{key}}
```

## 找出占内存最大的键

采样扫描，对线上影响很小。

```sh @tags=diagnose
redis-cli -h {{host}} --bigkeys
```

## 找出访问最频繁的键

```sh @tags=diagnose
redis-cli -h {{host}} --hotkeys
```

## 查看某个键占了多少内存

```sh @tags=diagnose
redis-cli -h {{host}} MEMORY USAGE {{key}}
```

## 查看整体内存情况

```sh @tags=diagnose
redis-cli -h {{host}} INFO memory
```

## 查看连接数和客户端

```sh @tags=diagnose
redis-cli -h {{host}} INFO clients
```

## 查看命中率

`keyspace_hits / (hits + misses)`，低于 0.9 说明缓存策略要调。

```sh @tags=diagnose
redis-cli -h {{host}} INFO stats | grep keyspace
```

## 实时看正在执行的命令

排查问题很好用，但输出量大，别开太久。

```sh @tags=diagnose @remote
redis-cli -h {{host}} MONITOR
```

## 查看慢查询日志

```sh @tags=diagnose
redis-cli -h {{host}} SLOWLOG GET 20
```

## 清空慢查询日志

```sh @tags=diagnose
redis-cli -h {{host}} SLOWLOG RESET
```

## 实时延迟采样

```sh @tags=diagnose
redis-cli -h {{host}} --latency
```

## 查看每个库有多少键

```sh @tags=diagnose
redis-cli -h {{host}} INFO keyspace
```

## 查看所有配置

```sh
redis-cli -h {{host}} CONFIG GET "*"
```

## 查看单个配置项

```sh
redis-cli -h {{host}} CONFIG GET {{param}}
```

## 运行时修改配置

重启后失效，要持久化得用 CONFIG REWRITE。

```sh @confirm
redis-cli -h {{host}} CONFIG SET {{param}} {{value}}
```

## 把运行时配置写回配置文件

```sh @confirm
redis-cli -h {{host}} CONFIG REWRITE
```

## 手动触发一次后台快照

```sh @tags=persist @confirm
redis-cli -h {{host}} BGSAVE
```

## 查看最后一次快照时间

```sh @tags=persist
redis-cli -h {{host}} LASTSAVE
```

## 重写 AOF 文件

AOF 涨太大时用。

```sh @tags=persist @confirm
redis-cli -h {{host}} BGREWRITEAOF
```

## 清空当前数据库

不可逆。

```sh @confirm
redis-cli -h {{host}} FLUSHDB ASYNC
```

## 清空所有数据库

不可逆，而且是所有库。

```sh @confirm
redis-cli -h {{host}} FLUSHALL ASYNC
```

## 查看主从复制状态

```sh @tags=replication
redis-cli -h {{host}} INFO replication
```

## 集群状态

```sh @tags=cluster
redis-cli -h {{host}} CLUSTER INFO
```

## 集群节点列表

```sh @tags=cluster
redis-cli -h {{host}} CLUSTER NODES
```

## 用 Docker 起一个本地 Redis

```sh @tags=docker
docker run -d --name redis -p 6379:6379 redis:7-alpine redis-server --appendonly yes
```

## 压测

```sh
redis-benchmark -h {{host}} -n 10000 -c 50 -q
```
