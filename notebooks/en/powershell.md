---
name: powershell
description: Windows / PowerShell - processes, ports, services, files, environment
tags: [windows, shell]
platform: [windows]
vars:
  proc:
    desc: process
    from: shell
    cmd: Get-Process | Sort-Object -Property WS -Descending | Select-Object -First 40 -ExpandProperty ProcessName
  svc:
    desc: service
    from: shell
    cmd: Get-Service | Select-Object -ExpandProperty Name
---

## Find what is using a port

The first thing to run when a service reports the port is already in use.

```ps1 @tags=daily @tags=port
Get-NetTCPConnection -LocalPort {{port}} -ErrorAction SilentlyContinue | Select-Object LocalAddress,LocalPort,State,@{n='Process';e={(Get-Process -Id $_.OwningProcess).ProcessName}},OwningProcess
```

## Kill whatever is holding a port

```ps1 @tags=port @confirm
Get-NetTCPConnection -LocalPort {{port}} | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }
```

## List every listening port

```ps1 @tags=port
Get-NetTCPConnection -State Listen | Sort-Object LocalPort | Select-Object LocalAddress,LocalPort,@{n='Process';e={(Get-Process -Id $_.OwningProcess).ProcessName}}
```

## Find a process by name

```ps1
Get-Process {{proc}} | Select-Object Id,ProcessName,WS,CPU,Path
```

## Kill a process by PID

```ps1 @confirm
Stop-Process -Id {{pid}} -Force
```

## Kill every process with a given name

```ps1 @confirm
Get-Process {{proc}} -ErrorAction SilentlyContinue | Stop-Process -Force
```

## The 20 processes using the most memory

```ps1
Get-Process | Sort-Object WS -Descending | Select-Object -First 20 ProcessName,Id,@{n='MB';e={[math]::Round($_.WS/1MB,1)}}
```

## Show a service's status

```ps1 @tags=service
Get-Service {{svc}} | Select-Object Name,DisplayName,Status,StartType
```

## Restart a service

```ps1 @tags=service @confirm
Restart-Service {{svc}} -Force
```

## Set a service to start automatically

```ps1 @tags=service @confirm
Set-Service {{svc}} -StartupType Automatic
```

## Set an environment variable for this session

Only applies to this terminal window and vanishes when you close it.

```ps1 @tags=env
$env:{{name}} = "{{value}}"
```

## Set a user environment variable permanently

Only new terminals pick it up; the current window will not.

```ps1 @tags=env
[Environment]::SetEnvironmentVariable("{{name}}", "{{value}}", "User")
```

## Show one environment variable

```ps1 @tags=env
$env:{{name}}
```

## Print PATH one entry per line

For when PATH is too long to read.

```ps1 @tags=env
$env:PATH -split ';' | Where-Object { $_ }
```

## Append a directory to the user PATH

```ps1 @tags=env @confirm
$old = [Environment]::GetEnvironmentVariable("PATH","User"); [Environment]::SetEnvironmentVariable("PATH", "$old;{{dir}}", "User")
```

## Find files recursively

```ps1 @tags=find
Get-ChildItem -Path {{dir}} -Recurse -Filter "{{pattern}}" -ErrorAction SilentlyContinue | Select-Object FullName,Length,LastWriteTime
```

## Search inside file contents

The equivalent of grep -rn.

```ps1 @tags=find
Get-ChildItem -Path {{dir}} -Recurse -File -ErrorAction SilentlyContinue | Select-String -Pattern "{{pattern}}" | Select-Object Path,LineNumber,Line
```

## Search only files with certain extensions

```ps1 @tags=find
Get-ChildItem -Path {{dir}} -Recurse -Include *.cs,*.json -ErrorAction SilentlyContinue | Select-String -Pattern "{{pattern}}"
```

## The 20 largest files below the current directory

```ps1 @tags=disk
Get-ChildItem -Recurse -File -ErrorAction SilentlyContinue | Sort-Object Length -Descending | Select-Object -First 20 @{n='MB';e={[math]::Round($_.Length/1MB,1)}},FullName
```

## Show how much space each subdirectory uses

For hunting down what filled the disk.

```ps1 @tags=disk
Get-ChildItem -Directory | ForEach-Object { [PSCustomObject]@{ Dir=$_.Name; MB=[math]::Round((Get-ChildItem $_.FullName -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum/1MB,1) } } | Sort-Object MB -Descending
```

## Show free disk space

```ps1 @tags=disk
Get-PSDrive -PSProvider FileSystem | Select-Object Name,@{n='UsedGB';e={[math]::Round($_.Used/1GB,1)}},@{n='FreeGB';e={[math]::Round($_.Free/1GB,1)}}
```

## Delete a directory and everything in it

Far faster than Explorer for node_modules, bin and obj.

```ps1 @confirm
Remove-Item -Path {{dir}} -Recurse -Force
```

## Delete every node_modules recursively

```ps1 @confirm
Get-ChildItem -Path . -Include node_modules -Recurse -Directory | Remove-Item -Recurse -Force
```

## Delete every bin and obj recursively

```ps1 @confirm
Get-ChildItem -Path . -Include bin,obj -Recurse -Directory | Remove-Item -Recurse -Force
```

## Extract a zip

```ps1
Expand-Archive -Path {{zip}} -DestinationPath {{dest}} -Force
```

## Create a zip

```ps1
Compress-Archive -Path {{src}} -DestinationPath {{zip}} -Force
```

## Hash a file

For verifying a downloaded installer.

```ps1
Get-FileHash {{file}} -Algorithm SHA256
```

## Download a file

```ps1
Invoke-WebRequest -Uri {{url}} -OutFile {{out}}
```

## Test whether a host and port are reachable

The equivalent of telnet for testing a port, and the first choice on Windows.

```ps1 @tags=network
Test-NetConnection {{host}} -Port {{port}}
```

## Show this machine's IPv4 addresses

```ps1 @tags=network
Get-NetIPAddress -AddressFamily IPv4 | Select-Object InterfaceAlias,IPAddress
```

## Flush the DNS cache

For when you edited hosts, or a name resolves wrongly.

```ps1 @tags=network
Clear-DnsClientCache
```

## Open a new terminal as administrator

```ps1
Start-Process powershell -Verb RunAs
```

## Show the PowerShell profile path

```ps1 @tags=profile
$PROFILE
```

## Edit the PowerShell profile in Notepad

```ps1 @tags=profile
if (-not (Test-Path $PROFILE)) { New-Item -ItemType File -Path $PROFILE -Force }; notepad $PROFILE
```

## Reload the profile

Saves reopening the terminal after editing $PROFILE.

```ps1 @tags=profile
. $PROFILE
```

## Allow local scripts to run

Run once, the first time a .ps1 is blocked from running.

```ps1 @confirm
Set-ExecutionPolicy -Scope CurrentUser RemoteSigned
```

## Open the command history file

PSReadLine keeps every command you have typed in here.

```ps1 @tags=history
notepad (Get-PSReadLineOption).HistorySavePath
```

## Your 30 most-used commands

```ps1 @tags=history
Get-Content (Get-PSReadLineOption).HistorySavePath | Group-Object | Sort-Object Count -Descending | Select-Object -First 30 Count,Name
```

## Search for software with winget

```ps1 @tags=pkg
winget search {{name}}
```

## Install software with winget

```ps1 @tags=pkg
winget install --id {{id}} -e
```

## Upgrade everything installed

```ps1 @tags=pkg @confirm
winget upgrade --all
```

## Show the OS version and last boot time

```ps1
Get-ComputerInfo -Property OsName,OsVersion,OsLastBootUpTime,CsTotalPhysicalMemory
```

## Copy a command's output to the clipboard

```ps1
{{command}} | Set-Clipboard
```
