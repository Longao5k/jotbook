---
name: systemd
description: systemd —— 服务管理与 journal 日志
tags: [ops, linux]
platform: [linux]
vars:
  service:
    desc: 服务名
    from: profile
    cmd: systemctl list-units --type=service --no-legend --plain --no-pager | awk '{print $1}'
  since:
    desc: 起始时间
    from: ask
    options: ["10 min ago", "1 hour ago", "today", "yesterday"]
---

## 重启服务

改完配置后最常用的一条。

```sh @confirm @tags=daily
sudo systemctl restart {{service}}
```

## 查看服务状态

带最近 10 行日志，排查启动失败第一步。

```sh @tags=daily
systemctl status {{service}}
```

## 启动服务

```sh
sudo systemctl start {{service}}
```

## 停止服务

```sh @confirm
sudo systemctl stop {{service}}
```

## 平滑重载配置，不中断连接

服务必须实现了 reload 才有效，nginx 支持，多数 .NET / Node 服务不支持。

```sh
sudo systemctl reload {{service}}
```

## 尝试重载，不支持就重启

```sh @confirm
sudo systemctl reload-or-restart {{service}}
```

## 设置开机自启并立即启动

```sh
sudo systemctl enable --now {{service}}
```

## 取消开机自启并立即停止

```sh @confirm
sudo systemctl disable --now {{service}}
```

## 修改了 unit 文件后必须重载

改完 /etc/systemd/system/*.service 不执行这句，systemd 还在用旧的。

```sh @tags=daily
sudo systemctl daemon-reload
```

## 实时跟踪服务日志

```sh @tags=logs @remote
journalctl -u {{service}} -f
```

## 查看服务最近的日志

```sh @tags=logs @remote
journalctl -u {{service}} --since "{{since}}" --no-pager
```

## 只看错误级别以上的日志

```sh @tags=logs @remote
journalctl -u {{service}} -p err --no-pager -n 100
```

## 查看本次启动以来的日志

```sh @tags=logs @remote
journalctl -u {{service}} -b --no-pager
```

## 查看上一次启动的日志

机器意外重启后查原因用。

```sh @tags=logs @remote
journalctl -b -1 -p err --no-pager
```

## 日志转成 JSON 输出

```sh @tags=logs
journalctl -u {{service}} -o json-pretty -n 20
```

## 编辑 unit 文件

会自动在保存后提示 daemon-reload。

```sh
sudo systemctl edit --full {{service}}
```

## 只覆盖部分配置，不改原文件

生成 override.conf，升级包时不会被覆盖，比直接改原文件安全。

```sh
sudo systemctl edit {{service}}
```

## 查看 unit 文件的完整内容

```sh
systemctl cat {{service}}
```

## 查看服务的所有生效配置项

```sh
systemctl show {{service}}
```

## 列出所有服务

```sh
systemctl list-units --type=service --no-pager
```

## 列出所有启动失败的服务

```sh @tags=daily
systemctl --failed
```

## 列出开机自启的服务

```sh
systemctl list-unit-files --type=service --state=enabled --no-pager
```

## 判断服务是否在运行

写脚本时用，只输出 active / inactive。

```sh
systemctl is-active {{service}}
```

## 查看开机耗时排行

```sh
systemd-analyze blame | head -20
```

## 查看 journal 占了多少磁盘

```sh
journalctl --disk-usage
```

## 清理 7 天前的日志

服务器磁盘被日志撑满时的救命命令。

```sh @confirm
sudo journalctl --vacuum-time=7d
```

## 把 journal 限制在 500M 以内

```sh @confirm
sudo journalctl --vacuum-size=500M
```

## 查看某个端口被哪个服务占用

```sh
sudo ss -lntp | grep :{{port}}
```

## 重启后查看服务是否自动拉起

```sh
systemctl is-enabled {{service}}
```

## 一个最小可用的 unit 文件

放到 /etc/systemd/system/{{service}}，然后 daemon-reload + enable --now。

```ini @tags=reference
[Unit]
Description={{description}}
After=network.target

[Service]
Type=notify
WorkingDirectory={{workdir}}
ExecStart={{exec}}
Restart=always
RestartSec=5
User={{user}}
Environment=ASPNETCORE_ENVIRONMENT=Production

[Install]
WantedBy=multi-user.target
```
