---
name: ssh
description: SSH —— 登录、密钥、端口转发、文件同步
tags: [ops, network]
vars:
  host:
    desc: 主机
    from: profile
    cmd: grep -i "^Host " ~/.ssh/config | awk '{print $2}' | grep -v "[*?]"
---

## 登录

```sh @tags=daily
ssh {{host}}
```

## 指定用户和端口登录

```sh
ssh -p {{port}} {{user}}@{{host}}
```

## 生成 ed25519 密钥

现在应该用 ed25519 而不是 rsa，更短更快更安全。

```sh @tags=key
ssh-keygen -t ed25519 -C "{{comment}}"
```

## 把公钥装到服务器上

```sh @platform=linux,macos @tags=key
ssh-copy-id {{user}}@{{host}}
```

## 把公钥装到服务器上（Windows）

Windows 没有 ssh-copy-id，用这句代替。

```ps1 @platform=windows @tags=key
type "$env:USERPROFILE\.ssh\id_ed25519.pub" | ssh {{user}}@{{host}} "mkdir -p ~/.ssh && cat >> ~/.ssh/authorized_keys"
```

## 查看自己的公钥

```ps1 @platform=windows @tags=key
Get-Content "$env:USERPROFILE\.ssh\id_ed25519.pub"
```

## 查看自己的公钥（bash）

```sh @platform=linux,macos @tags=key
cat ~/.ssh/id_ed25519.pub
```

## 指定密钥文件登录

```sh @tags=key
ssh -i {{keyfile}} {{user}}@{{host}}
```

## 服务器换了系统，提示 host key 冲突

删掉旧记录再连即可，但如果不是你自己重装的，要警惕中间人。

```sh
ssh-keygen -R {{host}}
```

## 测试连通性，不真正登录

```sh
ssh -T {{user}}@{{host}}
```

## 排查连不上的原因

打印完整握手过程，认证失败时看这个。

```sh
ssh -vvv {{host}}
```

## 本地端口转发：把远端服务映射到本地

访问只在服务器内网开放的数据库、后台时用。之后浏览器打开 localhost:本地端口。

```sh @tags=tunnel
ssh -N -L {{localport}}:localhost:{{remoteport}} {{host}}
```

## 本地端口转发到第三方主机

服务器能访问、但你访问不了的机器。

```sh @tags=tunnel
ssh -N -L {{localport}}:{{target}}:{{remoteport}} {{host}}
```

## 远程端口转发：把本地服务暴露给服务器

本地开发机的服务给服务器调用，做 webhook 调试很好用。

```sh @tags=tunnel
ssh -N -R {{remoteport}}:localhost:{{localport}} {{host}}
```

## 建立 SOCKS5 代理

浏览器设置 socks5://127.0.0.1:1080 后即可用服务器的网络出口。

```sh @tags=tunnel
ssh -N -D 1080 {{host}}
```

## 隧道放到后台运行

```sh @tags=tunnel
ssh -f -N -L {{localport}}:localhost:{{remoteport}} {{host}}
```

## 上传单个文件

```sh @tags=copy
scp {{file}} {{host}}:{{dest}}
```

## 下载单个文件

```sh @tags=copy
scp {{host}}:{{src}} .
```

## 上传整个目录

```sh @tags=copy
scp -r {{dir}} {{host}}:{{dest}}
```

## 用 rsync 同步目录，只传差异

大目录比 scp 快得多，中断了可以续传。

```sh @tags=copy
rsync -avz --progress {{src}} {{host}}:{{dest}}
```

## rsync 排除 node_modules 等目录

```sh @tags=copy
rsync -avz --progress --exclude 'node_modules' --exclude '.git' --exclude 'bin' --exclude 'obj' {{src}} {{host}}:{{dest}}
```

## rsync 并删除目标端多余的文件

让目标端和源端完全一致，会删文件，先用 --dry-run 预览。

```sh @tags=copy @confirm
rsync -avz --delete {{src}} {{host}}:{{dest}}
```

## 预览 rsync 会做什么

```sh @tags=copy
rsync -avzn --delete {{src}} {{host}}:{{dest}}
```

## 在远端执行一条命令后立即返回

```sh
ssh {{host}} "{{command}}"
```

## 在远端用 sudo 执行命令

```sh @confirm
ssh -t {{host}} "sudo {{command}}"
```

## 把本地文件内容通过管道写到远端

```sh
cat {{file}} | ssh {{host}} "cat > {{dest}}"
```

## 一个好用的 ssh config 模板

放在 ~/.ssh/config，之后直接 ssh prod 即可。ServerAliveInterval 防止挂着不动被断开。

```ini @tags=reference
Host prod
    HostName 1.2.3.4
    User deploy
    Port 22
    IdentityFile ~/.ssh/id_ed25519
    ServerAliveInterval 30
    ServerAliveCountMax 6

Host *
    AddKeysToAgent yes
    Compression yes
```

## 修复私钥权限过于开放的报错

Linux / macOS 上私钥必须是 600，否则 ssh 拒绝使用。

```sh @platform=linux,macos @tags=key
chmod 600 ~/.ssh/id_ed25519 && chmod 700 ~/.ssh
```

## 启动 ssh-agent 并加载密钥（PowerShell）

```ps1 @platform=windows @tags=key
Start-Service ssh-agent
ssh-add "$env:USERPROFILE\.ssh\id_ed25519"
```

## 启动 ssh-agent 并加载密钥（bash）

```sh @platform=linux,macos @tags=key
eval "$(ssh-agent -s)" && ssh-add ~/.ssh/id_ed25519
```
