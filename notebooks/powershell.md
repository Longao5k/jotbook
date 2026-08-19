---
name: powershell
description: Windows / PowerShell —— 进程、端口、服务、文件、环境变量
tags: [windows, shell]
platform: [windows]
vars:
  proc:
    desc: 进程
    from: shell
    cmd: Get-Process | Sort-Object -Property WS -Descending | Select-Object -First 40 -ExpandProperty ProcessName
  svc:
    desc: 服务
    from: shell
    cmd: Get-Service | Select-Object -ExpandProperty Name
---

## 查看谁占用了某个端口

启动服务报「端口已被占用」时的第一条命令。

```ps1 @tags=daily @tags=port
Get-NetTCPConnection -LocalPort {{port}} -ErrorAction SilentlyContinue | Select-Object LocalAddress,LocalPort,State,@{n='Process';e={(Get-Process -Id $_.OwningProcess).ProcessName}},OwningProcess
```

## 杀掉占用某端口的进程

```ps1 @tags=port @confirm
Get-NetTCPConnection -LocalPort {{port}} | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }
```

## 查看所有监听中的端口

```ps1 @tags=port
Get-NetTCPConnection -State Listen | Sort-Object LocalPort | Select-Object LocalAddress,LocalPort,@{n='Process';e={(Get-Process -Id $_.OwningProcess).ProcessName}}
```

## 按名字查进程

```ps1
Get-Process {{proc}} | Select-Object Id,ProcessName,WS,CPU,Path
```

## 按 PID 杀进程

```ps1 @confirm
Stop-Process -Id {{pid}} -Force
```

## 按名字杀掉所有同名进程

```ps1 @confirm
Get-Process {{proc}} -ErrorAction SilentlyContinue | Stop-Process -Force
```

## 查看内存占用最高的 20 个进程

```ps1
Get-Process | Sort-Object WS -Descending | Select-Object -First 20 ProcessName,Id,@{n='MB';e={[math]::Round($_.WS/1MB,1)}}
```

## 查看服务状态

```ps1 @tags=service
Get-Service {{svc}} | Select-Object Name,DisplayName,Status,StartType
```

## 重启服务

```ps1 @tags=service @confirm
Restart-Service {{svc}} -Force
```

## 设置服务为自动启动

```ps1 @tags=service @confirm
Set-Service {{svc}} -StartupType Automatic
```

## 设置当前会话的环境变量

只在这个终端窗口有效，关掉就没了。

```ps1 @tags=env
$env:{{name}} = "{{value}}"
```

## 永久设置用户级环境变量

新开的终端才会生效，当前窗口不会。

```ps1 @tags=env
[Environment]::SetEnvironmentVariable("{{name}}", "{{value}}", "User")
```

## 查看某个环境变量

```ps1 @tags=env
$env:{{name}}
```

## 把 PATH 分行显示

PATH 太长看不清时用。

```ps1 @tags=env
$env:PATH -split ';' | Where-Object { $_ }
```

## 往用户 PATH 里追加一个目录

```ps1 @tags=env @confirm
$old = [Environment]::GetEnvironmentVariable("PATH","User"); [Environment]::SetEnvironmentVariable("PATH", "$old;{{dir}}", "User")
```

## 递归查找文件

```ps1 @tags=find
Get-ChildItem -Path {{dir}} -Recurse -Filter "{{pattern}}" -ErrorAction SilentlyContinue | Select-Object FullName,Length,LastWriteTime
```

## 在文件内容里搜索文本

相当于 grep -rn。

```ps1 @tags=find
Get-ChildItem -Path {{dir}} -Recurse -File -ErrorAction SilentlyContinue | Select-String -Pattern "{{pattern}}" | Select-Object Path,LineNumber,Line
```

## 只在指定后缀的文件里搜索

```ps1 @tags=find
Get-ChildItem -Path {{dir}} -Recurse -Include *.cs,*.json -ErrorAction SilentlyContinue | Select-String -Pattern "{{pattern}}"
```

## 找出当前目录下最大的 20 个文件

```ps1 @tags=disk
Get-ChildItem -Recurse -File -ErrorAction SilentlyContinue | Sort-Object Length -Descending | Select-Object -First 20 @{n='MB';e={[math]::Round($_.Length/1MB,1)}},FullName
```

## 统计各子目录占用的空间

磁盘满了找元凶用。

```ps1 @tags=disk
Get-ChildItem -Directory | ForEach-Object { [PSCustomObject]@{ Dir=$_.Name; MB=[math]::Round((Get-ChildItem $_.FullName -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum/1MB,1) } } | Sort-Object MB -Descending
```

## 查看磁盘剩余空间

```ps1 @tags=disk
Get-PSDrive -PSProvider FileSystem | Select-Object Name,@{n='UsedGB';e={[math]::Round($_.Used/1GB,1)}},@{n='FreeGB';e={[math]::Round($_.Free/1GB,1)}}
```

## 删除目录及其全部内容

node_modules、bin、obj 用这个删比资源管理器快得多。

```ps1 @confirm
Remove-Item -Path {{dir}} -Recurse -Force
```

## 递归删除所有 node_modules

```ps1 @confirm
Get-ChildItem -Path . -Include node_modules -Recurse -Directory | Remove-Item -Recurse -Force
```

## 递归删除所有 bin 和 obj

```ps1 @confirm
Get-ChildItem -Path . -Include bin,obj -Recurse -Directory | Remove-Item -Recurse -Force
```

## 解压 zip

```ps1
Expand-Archive -Path {{zip}} -DestinationPath {{dest}} -Force
```

## 压缩成 zip

```ps1
Compress-Archive -Path {{src}} -DestinationPath {{zip}} -Force
```

## 计算文件哈希

校验下载的安装包。

```ps1
Get-FileHash {{file}} -Algorithm SHA256
```

## 下载文件

```ps1
Invoke-WebRequest -Uri {{url}} -OutFile {{out}}
```

## 测试到某主机某端口是否通

相当于 telnet 测端口，Windows 上首选这个。

```ps1 @tags=network
Test-NetConnection {{host}} -Port {{port}}
```

## 查看本机 IPv4 地址

```ps1 @tags=network
Get-NetIPAddress -AddressFamily IPv4 | Select-Object InterfaceAlias,IPAddress
```

## 清空 DNS 缓存

改了 hosts 或域名解析不对时用。

```ps1 @tags=network
Clear-DnsClientCache
```

## 用管理员权限开一个新终端

```ps1
Start-Process powershell -Verb RunAs
```

## 查看 PowerShell 配置文件路径

```ps1 @tags=profile
$PROFILE
```

## 用记事本编辑 PowerShell 配置文件

```ps1 @tags=profile
if (-not (Test-Path $PROFILE)) { New-Item -ItemType File -Path $PROFILE -Force }; notepad $PROFILE
```

## 重新加载配置文件

改完 $PROFILE 不用重开终端。

```ps1 @tags=profile
. $PROFILE
```

## 允许运行本地脚本

第一次跑 .ps1 报「禁止运行脚本」时执行一次即可。

```ps1 @confirm
Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
```

## 打开命令历史文件

PSReadLine 把你敲过的每条命令都存在这里。

```ps1 @tags=history
notepad (Get-PSReadLineOption).HistorySavePath
```

## 统计最常用的 30 条命令

```ps1 @tags=history
Get-Content (Get-PSReadLineOption).HistorySavePath | Group-Object | Sort-Object Count -Descending | Select-Object -First 30 Count,Name
```

## 用 winget 搜索软件

```ps1 @tags=pkg
winget search {{name}}
```

## 用 winget 安装软件

```ps1 @tags=pkg
winget install --id {{id}} -e
```

## 升级所有已安装的软件

```ps1 @tags=pkg @confirm
winget upgrade --all
```

## 查看系统版本和启动时间

```ps1
Get-ComputerInfo -Property OsName,OsVersion,OsLastBootUpTime,CsTotalPhysicalMemory
```

## 复制命令输出到剪贴板

```ps1
{{command}} | Set-Clipboard
```
