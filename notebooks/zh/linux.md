---
name: linux
description: Linux —— 进程、端口、磁盘、查找、权限、压缩
tags: [ops, shell]
platform: [linux, macos]
---

## 查看谁占用了某个端口

```sh @tags=daily @tags=port
sudo ss -lntp | grep :{{port}}
```

## 列出所有监听中的端口

```sh @tags=port
sudo ss -lntup
```

## 查看某个进程

```sh @tags=proc
ps aux | grep -i {{name}} | grep -v grep
```

## 按 PID 结束进程

```sh @tags=proc @confirm
kill {{pid}}
```

## 强制结束进程

普通 kill 无效时才用，进程来不及做清理。

```sh @tags=proc @confirm
kill -9 {{pid}}
```

## 按名字模糊匹配并结束

```sh @tags=proc @confirm
pkill -f {{pattern}}
```

## 查看进程树

```sh @tags=proc
ps -ef --forest | head -50
```

## 查看 CPU 占用最高的进程

```sh @tags=proc
ps aux --sort=-%cpu | head -15
```

## 查看内存占用最高的进程

```sh @tags=proc
ps aux --sort=-%mem | head -15
```

## 查看磁盘剩余空间

```sh @tags=disk @tags=daily
df -h
```

## 查看当前目录各子目录占用

磁盘满了逐层往下找的标准姿势。

```sh @tags=disk
du -h --max-depth=1 {{dir}} 2>/dev/null | sort -hr | head -20
```

## 找出大于指定体积的文件

```sh @tags=disk
find {{dir}} -type f -size +{{size}}M -exec ls -lh {} \; 2>/dev/null | awk '{print $5, $9}'
```

## 查看 inode 使用情况

磁盘明明有空间却报 No space left，通常是 inode 用光了。

```sh @tags=disk
df -i
```

## 按文件名查找

```sh @tags=find
find {{dir}} -name "{{pattern}}" 2>/dev/null
```

## 按内容递归搜索

```sh @tags=find
grep -rn --color=always "{{pattern}}" {{dir}}
```

## 只在指定后缀里搜索

```sh @tags=find
grep -rn --include="*.{{ext}}" "{{pattern}}" {{dir}}
```

## 搜索时排除目录

```sh @tags=find
grep -rn --exclude-dir={node_modules,.git,bin,obj} "{{pattern}}" {{dir}}
```

## 查找最近修改过的文件

```sh @tags=find
find {{dir}} -type f -mtime -{{days}} -printf '%TY-%Tm-%Td %TH:%TM %p\n' 2>/dev/null | sort -r | head -30
```

## 实时跟踪日志文件

```sh @tags=logs @remote
tail -f {{file}}
```

## 跟踪日志并只看包含关键字的行

```sh @tags=logs @remote
tail -f {{file}} | grep --line-buffered -i "{{pattern}}"
```

## 查看文件的最后 200 行

```sh @tags=logs
tail -n 200 {{file}}
```

## 查看内存使用

```sh
free -h
```

## 查看系统负载和运行时长

```sh
uptime
```

## 查看系统版本

```sh
cat /etc/os-release
```

## 查看 CPU 信息

```sh
lscpu | head -20
```

## 解压 tar.gz

```sh @tags=archive
tar -xzf {{file}}
```

## 解压到指定目录

```sh @tags=archive
tar -xzf {{file}} -C {{dir}}
```

## 打包成 tar.gz

```sh @tags=archive
tar -czf {{out}}.tar.gz {{dir}}
```

## 打包时排除某些目录

```sh @tags=archive
tar -czf {{out}}.tar.gz --exclude='node_modules' --exclude='.git' {{dir}}
```

## 查看压缩包内容而不解压

```sh @tags=archive
tar -tzf {{file}} | head -50
```

## 解压 zip

```sh @tags=archive
unzip {{file}} -d {{dir}}
```

## 给文件加执行权限

```sh @tags=perm
chmod +x {{file}}
```

## 递归修改目录属主

部署时文件属主不对导致服务读不到，很常见。

```sh @tags=perm @confirm
sudo chown -R {{user}}:{{user}} {{dir}}
```

## 递归设置目录 755 文件 644

```sh @tags=perm @confirm
find {{dir}} -type d -exec chmod 755 {} \; && find {{dir}} -type f -exec chmod 644 {} \;
```

## 创建软链接

```sh
ln -s {{target}} {{link}}
```

## 查看是谁在占用某个文件

删不掉文件、卸载不了目录时用。

```sh
sudo lsof {{file}}
```

## 查看某进程打开的所有文件

```sh
sudo lsof -p {{pid}}
```

## 测试 HTTP 接口并显示响应头

```sh @tags=network
curl -i {{url}}
```

## 只看 HTTP 状态码和耗时

```sh @tags=network
curl -s -o /dev/null -w "code=%{http_code} time=%{time_total}s\n" {{url}}
```

## 发送 JSON POST 请求

```sh @tags=network
curl -X POST {{url}} -H "Content-Type: application/json" -d '{{json}}'
```

## 带 Bearer Token 请求

```sh @tags=network
curl {{url}} -H "Authorization: Bearer {{token}}"
```

## 下载文件并显示进度

```sh @tags=network
curl -L -o {{out}} {{url}}
```

## 查看域名解析结果

```sh @tags=network
dig +short {{domain}}
```

## 编辑当前用户的定时任务

```sh @tags=cron
crontab -e
```

## 查看当前用户的定时任务

```sh @tags=cron
crontab -l
```

## 常用 cron 表达式速查

分 时 日 月 周。

```txt @tags=reference
*/5 * * * *     每 5 分钟
0 * * * *       每小时整点
0 3 * * *       每天凌晨 3 点
0 3 * * 1       每周一凌晨 3 点
0 3 1 * *       每月 1 号凌晨 3 点
```

## 查看最近登录记录

```sh
last -n 20
```

## 同步系统时间

```sh @confirm
sudo timedatectl set-ntp true && timedatectl status
```
