---
name: docker
description: Docker —— 容器、镜像、compose、磁盘清理
tags: [ops, container]
vars:
  container:
    desc: 容器
    from: shell
    cmd: docker ps -a --format "{{.Names}}\t{{.Status}}"
  image:
    desc: 镜像
    from: shell
    cmd: docker images --format "{{.Repository}}:{{.Tag}}"
  service:
    desc: compose 服务
    from: shell
    cmd: docker compose config --services
  sh:
    desc: 容器内的 shell
    from: ask
    options: ["sh", "bash"]
---

## 列出运行中的容器

```sh @tags=daily
docker ps
```

## 列出所有容器，包括已停止的

```sh @tags=daily
docker ps -a
```

## 只看容器名和状态

```sh
docker ps -a --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
```

## 进入容器

镜像基于 alpine 时只有 sh，没有 bash。

```sh @tags=daily
docker exec -it {{container}} {{sh}}
```

## 以 root 身份进入容器

容器里没权限装东西时用。

```sh
docker exec -it -u root {{container}} {{sh}}
```

## 跟踪容器日志

```sh @tags=daily @tags=logs
docker logs -f --tail 200 {{container}}
```

## 查看容器最近的日志并带时间戳

```sh @tags=logs
docker logs --timestamps --since 30m {{container}}
```

## 重启容器

```sh @confirm
docker restart {{container}}
```

## 停止并删除容器

```sh @confirm
docker rm -f {{container}}
```

## 查看容器的 IP 地址

```sh
docker inspect -f '\{{range .NetworkSettings.Networks}}\{{.IPAddress}}\{{end}}' {{container}}
```

## 查看容器的环境变量

```sh
docker inspect -f '\{{range .Config.Env}}\{{println .}}\{{end}}' {{container}}
```

## 查看容器实时资源占用

```sh
docker stats
```

## 从容器里拷文件出来

```sh
docker cp {{container}}:{{src}} {{dest}}
```

## 往容器里拷文件

```sh
docker cp {{src}} {{container}}:{{dest}}
```

## 构建镜像

```sh @tags=build
docker build -t {{image}} .
```

## 构建时不使用缓存

Dockerfile 没改但依赖变了、构建结果不对时用。

```sh @tags=build
docker build --no-cache -t {{image}} .
```

## 构建时传入参数

```sh @tags=build
docker build --build-arg {{key}}={{value}} -t {{image}} .
```

## 构建多平台镜像

```sh @tags=build
docker buildx build --platform linux/amd64,linux/arm64 -t {{image}} --push .
```

## 后台运行并映射端口

```sh
docker run -d --name {{name}} -p {{hostport}}:{{containerport}} {{image}}
```

## 运行一次性容器，退出即删除

```sh
docker run --rm -it {{image}} {{sh}}
```

## 运行并挂载当前目录

```sh @platform=linux,macos
docker run --rm -it -v "$(pwd)":/app -w /app {{image}} {{sh}}
```

## 运行并挂载当前目录（PowerShell）

```ps1 @platform=windows
docker run --rm -it -v "${PWD}:/app" -w /app {{image}} {{sh}}
```

## 查看镜像的构建层

排查镜像为什么这么大。

```sh
docker history {{image}} --no-trunc
```

## 查看 Docker 占了多少磁盘

```sh @tags=cleanup
docker system df
```

## 清理停止的容器、悬空镜像、未使用网络和构建缓存

日常清理，不会动正在使用的东西。

```sh @tags=cleanup @confirm
docker system prune -f
```

## 深度清理，包括所有未被容器使用的镜像

会删掉你只是暂时没跑的镜像，下次要重新拉。

```sh @tags=cleanup @confirm
docker system prune -af
```

## 清理未使用的数据卷

数据卷里可能有数据库文件，删前确认。

```sh @tags=cleanup @confirm
docker volume prune -f
```

## 只清理构建缓存

buildx 缓存经常悄悄涨到几十 G。

```sh @tags=cleanup
docker builder prune -af
```

## 启动 compose 全部服务

```sh @tags=compose @tags=daily
docker compose up -d
```

## 重建并启动单个服务

改完某个服务的代码后用。

```sh @tags=compose
docker compose up -d --build {{service}}
```

## 跟踪 compose 日志

```sh @tags=compose @tags=logs
docker compose logs -f --tail 200 {{service}}
```

## 停止 compose 全部服务

```sh @tags=compose
docker compose down
```

## 停止并删除数据卷

数据库数据会一起没，慎用。

```sh @tags=compose @confirm
docker compose down -v
```

## 查看 compose 展开后的完整配置

排查环境变量没生效、YAML 锚点写错。

```sh @tags=compose
docker compose config
```

## 在 compose 服务里执行命令

```sh @tags=compose
docker compose exec {{service}} {{sh}}
```

## 拉取 compose 依赖的所有镜像

```sh @tags=compose
docker compose pull
```

## 登录私有镜像仓库

```sh
docker login {{registry}}
```

## 给镜像打标签并推送

```sh
docker tag {{image}} {{registry}}/{{image}} && docker push {{registry}}/{{image}}
```

## 把镜像导出成文件

内网机器没法拉镜像时，用这个搬运。

```sh
docker save -o {{file}}.tar {{image}}
```

## 从文件导入镜像

```sh
docker load -i {{file}}.tar
```
