---
name: nginx
description: nginx —— 配置校验、平滑重载、日志与反向代理排查
tags: [ops, web]
platform: [linux, macos]
vars:
  site:
    desc: 站点配置名
    from: shell
    cmd: ls /etc/nginx/sites-available 2>/dev/null || ls /etc/nginx/conf.d 2>/dev/null
---

## 校验配置

**改完配置永远先跑这一条**，语法错了直接 reload 会让 nginx 起不来。

```sh @tags=daily
sudo nginx -t
```

## 平滑重载配置

不断连接，正在处理的请求会走完。日常改配置用这个，不要 restart。

```sh @tags=daily
sudo nginx -t && sudo systemctl reload nginx
```

## 重启（会断连接）

只有改了 worker 数量、监听端口这类启动参数才需要。

```sh @confirm
sudo systemctl restart nginx
```

## 查看运行状态

```sh
systemctl status nginx
```

## 查看编译时带了哪些模块

排查某个指令「unknown directive」时用。

```sh
nginx -V
```

## 查看合并后的完整配置

include 展开之后的样子，排查配置到底哪一段生效。

```sh
sudo nginx -T
```

## 实时跟踪访问日志

```sh @tags=logs @remote
sudo tail -f /var/log/nginx/access.log
```

## 实时跟踪错误日志

502/504 先看这里。

```sh @tags=logs @remote
sudo tail -f /var/log/nginx/error.log
```

## 只看某个状态码的请求

```sh @tags=logs
sudo awk '$9 == {{code}}' /var/log/nginx/access.log | tail -50
```

## 统计各状态码出现次数

```sh @tags=logs
sudo awk '{print $9}' /var/log/nginx/access.log | sort | uniq -c | sort -rn
```

## 统计访问量最高的 URL

```sh @tags=logs
sudo awk '{print $7}' /var/log/nginx/access.log | sort | uniq -c | sort -rn | head -20
```

## 统计请求量最高的 IP

排查刷接口用。

```sh @tags=logs
sudo awk '{print $1}' /var/log/nginx/access.log | sort | uniq -c | sort -rn | head -20
```

## 找出最慢的请求

需要日志格式里带了 $request_time。

```sh @tags=logs
sudo awk '{print $NF, $7}' /var/log/nginx/access.log | sort -rn | head -20
```

## 启用一个站点（Debian/Ubuntu 布局）

```sh
sudo ln -s /etc/nginx/sites-available/{{site}} /etc/nginx/sites-enabled/ && sudo nginx -t && sudo systemctl reload nginx
```

## 停用一个站点

```sh @confirm
sudo rm /etc/nginx/sites-enabled/{{site}} && sudo nginx -t && sudo systemctl reload nginx
```

## 编辑站点配置

```sh
sudo nano /etc/nginx/sites-available/{{site}}
```

## 反向代理到本地服务的最小配置

.NET / Node 服务放在 nginx 后面时最常用的一段。proxy_set_header 那几行不写，
后端拿到的客户端 IP 和协议都是错的。

```nginx @tags=reference
server {
    listen 80;
    server_name {{domain}};

    location / {
        proxy_pass         http://127.0.0.1:{{port}};
        proxy_http_version 1.1;
        proxy_set_header   Host              $host;
        proxy_set_header   X-Real-IP         $remote_addr;
        proxy_set_header   X-Forwarded-For   $proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto $scheme;

        # WebSocket / SignalR 需要这两行
        proxy_set_header   Upgrade    $http_upgrade;
        proxy_set_header   Connection "upgrade";

        proxy_read_timeout 300s;
    }
}
```

## 静态站点的最小配置

```nginx @tags=reference
server {
    listen 80;
    server_name {{domain}};
    root /var/www/{{domain}};
    index index.html;

    # 前端路由（Vue/React/Flutter Web）刷新 404 就是缺这一行
    location / {
        try_files $uri $uri/ /index.html;
    }

    location ~* \.(js|css|png|jpg|svg|woff2)$ {
        expires 30d;
        add_header Cache-Control "public, immutable";
    }
}
```

## 用 certbot 申请并自动配置 HTTPS

```sh @confirm
sudo certbot --nginx -d {{domain}}
```

## 测试证书自动续期

```sh
sudo certbot renew --dry-run
```

## 查看证书到期时间

```sh
echo | openssl s_client -servername {{domain}} -connect {{domain}}:443 2>/dev/null | openssl x509 -noout -dates
```

## 查看 nginx 占用的连接数

需要开启 stub_status 模块。

```sh @tags=diagnose
curl -s http://127.0.0.1/nginx_status
```

## 查看 nginx 的 worker 进程

```sh @tags=diagnose
ps aux | grep '[n]ginx'
```

## 查看谁在占用 80/443

```sh @tags=diagnose
sudo ss -lntp | grep -E ':(80|443) '
```

## 清空日志但不重启

直接 rm 日志文件 nginx 不会重新创建，句柄还指着已删除的 inode。

```sh @confirm
sudo truncate -s 0 /var/log/nginx/access.log
```

## 限制单 IP 请求速率

```nginx @tags=reference
# http 段
limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;

# location 段
limit_req zone=api burst=20 nodelay;
```
