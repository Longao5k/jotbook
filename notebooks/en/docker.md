---
name: docker
description: Docker - containers, images, compose, reclaiming disk
tags: [ops, container]
vars:
  container:
    desc: container
    from: shell
    cmd: docker ps -a --format "{{.Names}}\t{{.Status}}"
  image:
    desc: image
    from: shell
    cmd: docker images --format "{{.Repository}}:{{.Tag}}"
  service:
    desc: compose service
    from: shell
    cmd: docker compose config --services
  sh:
    desc: shell inside the container
    from: ask
    options: ["sh", "bash"]
---

## List running containers

```sh @tags=daily
docker ps
```

## List every container, stopped ones included

```sh @tags=daily
docker ps -a
```

## Show just names and status

```sh
docker ps -a --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
```

## Get a shell in a container

Alpine-based images only have sh, not bash.

```sh @tags=daily
docker exec -it {{container}} {{sh}}
```

## Get a root shell in a container

For when you lack permission to install anything inside.

```sh
docker exec -it -u root {{container}} {{sh}}
```

## Follow a container's logs

```sh @tags=daily @tags=logs
docker logs -f --tail 200 {{container}}
```

## Recent logs with timestamps

```sh @tags=logs
docker logs --timestamps --since 30m {{container}}
```

## Restart a container

```sh @confirm
docker restart {{container}}
```

## Stop and remove a container

```sh @confirm
docker rm -f {{container}}
```

## Show a container's IP address

```sh
docker inspect -f '\{{range .NetworkSettings.Networks}}\{{.IPAddress}}\{{end}}' {{container}}
```

## Show a container's environment variables

```sh
docker inspect -f '\{{range .Config.Env}}\{{println .}}\{{end}}' {{container}}
```

## Live resource usage per container

```sh
docker stats
```

## Copy a file out of a container

```sh
docker cp {{container}}:{{src}} {{dest}}
```

## Copy a file into a container

```sh
docker cp {{src}} {{container}}:{{dest}}
```

## Build an image

```sh @tags=build
docker build -t {{image}} .
```

## Build without the cache

For when the Dockerfile has not changed but its dependencies have, and the build comes out wrong.

```sh @tags=build
docker build --no-cache -t {{image}} .
```

## Build with a build argument

```sh @tags=build
docker build --build-arg {{key}}={{value}} -t {{image}} .
```

## Build a multi-platform image

```sh @tags=build
docker buildx build --platform linux/amd64,linux/arm64 -t {{image}} --push .
```

## Run detached with a port mapping

```sh
docker run -d --name {{name}} -p {{hostport}}:{{containerport}} {{image}}
```

## Run a throwaway container, removed on exit

```sh
docker run --rm -it {{image}} {{sh}}
```

## Run with the current directory mounted

```sh @platform=linux,macos
docker run --rm -it -v "$(pwd)":/app -w /app {{image}} {{sh}}
```

## Run with the current directory mounted (PowerShell)

```ps1 @platform=windows
docker run --rm -it -v "${PWD}:/app" -w /app {{image}} {{sh}}
```

## Inspect an image's layers

For working out why an image is so large.

```sh
docker history {{image}} --no-trunc
```

## See how much disk Docker is using

```sh @tags=cleanup
docker system df
```

## Prune stopped containers, dangling images, unused networks and build cache

Routine cleanup; it will not touch anything in use.

```sh @tags=cleanup @confirm
docker system prune -f
```

## Deep clean, including every image no container uses

This removes images you have merely not run lately, and you will have to pull them again.

```sh @tags=cleanup @confirm
docker system prune -af
```

## Prune unused volumes

Volumes may hold database files, so check before confirming.

```sh @tags=cleanup @confirm
docker volume prune -f
```

## Prune the build cache only

The buildx cache quietly grows to tens of gigabytes.

```sh @tags=cleanup
docker builder prune -af
```

## Start every compose service

```sh @tags=compose @tags=daily
docker compose up -d
```

## Rebuild and restart one service

What you run after changing that service's code.

```sh @tags=compose
docker compose up -d --build {{service}}
```

## Follow compose logs

```sh @tags=compose @tags=logs
docker compose logs -f --tail 200 {{service}}
```

## Stop every compose service

```sh @tags=compose
docker compose down
```

## Stop and delete the volumes too

Database data goes with them. Use carefully.

```sh @tags=compose @confirm
docker compose down -v
```

## Show the fully expanded compose configuration

For chasing down env vars that did not apply, or a mistyped YAML anchor.

```sh @tags=compose
docker compose config
```

## Run a command in a compose service

```sh @tags=compose
docker compose exec {{service}} {{sh}}
```

## Pull every image compose depends on

```sh @tags=compose
docker compose pull
```

## Log in to a private registry

```sh
docker login {{registry}}
```

## Tag an image and push it

```sh
docker tag {{image}} {{registry}}/{{image}} && docker push {{registry}}/{{image}}
```

## Export an image to a file

For moving images onto machines that cannot reach a registry.

```sh
docker save -o {{file}}.tar {{image}}
```

## Load an image from a file

```sh
docker load -i {{file}}.tar
```
