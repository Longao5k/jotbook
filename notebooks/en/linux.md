---
name: linux
description: Linux - processes, ports, disk, search, permissions, archives
tags: [ops, shell]
platform: [linux, macos]
---

## Find what is using a port

```sh @tags=daily @tags=port
sudo ss -lntp | grep :{{port}}
```

## List every listening port

```sh @tags=port
sudo ss -lntup
```

## Find a process

```sh @tags=proc
ps aux | grep -i {{name}} | grep -v grep
```

## Kill a process by PID

```sh @tags=proc @confirm
kill {{pid}}
```

## Force-kill a process

Only when a plain kill does nothing; the process gets no chance to clean up.

```sh @tags=proc @confirm
kill -9 {{pid}}
```

## Kill by matching the command line

```sh @tags=proc @confirm
pkill -f {{pattern}}
```

## Show the process tree

```sh @tags=proc
ps -ef --forest | head -50
```

## Processes using the most CPU

```sh @tags=proc
ps aux --sort=-%cpu | head -15
```

## Processes using the most memory

```sh @tags=proc
ps aux --sort=-%mem | head -15
```

## Show free disk space

```sh @tags=disk @tags=daily
df -h
```

## Show what each subdirectory is using

The standard way to walk down a full disk one level at a time.

```sh @tags=disk
du -h --max-depth=1 {{dir}} 2>/dev/null | sort -hr | head -20
```

## Find files larger than a given size

```sh @tags=disk
find {{dir}} -type f -size +{{size}}M -exec ls -lh {} \; 2>/dev/null | awk '{print $5, $9}'
```

## Check inode usage

"No space left" while df shows free space usually means inodes ran out.

```sh @tags=disk
df -i
```

## Find files by name

```sh @tags=find
find {{dir}} -name "{{pattern}}" 2>/dev/null
```

## Search file contents recursively

```sh @tags=find
grep -rn --color=always "{{pattern}}" {{dir}}
```

## Search only files with a given extension

```sh @tags=find
grep -rn --include="*.{{ext}}" "{{pattern}}" {{dir}}
```

## Search while excluding directories

```sh @tags=find
grep -rn --exclude-dir={node_modules,.git,bin,obj} "{{pattern}}" {{dir}}
```

## Find recently modified files

```sh @tags=find
find {{dir}} -type f -mtime -{{days}} -printf '%TY-%Tm-%Td %TH:%TM %p\n' 2>/dev/null | sort -r | head -30
```

## Follow a log file

```sh @tags=logs @remote
tail -f {{file}}
```

## Follow a log and show only matching lines

```sh @tags=logs @remote
tail -f {{file}} | grep --line-buffered -i "{{pattern}}"
```

## Show the last 200 lines of a file

```sh @tags=logs
tail -n 200 {{file}}
```

## Show memory usage

```sh
free -h
```

## Show load average and uptime

```sh
uptime
```

## Show the OS version

```sh
cat /etc/os-release
```

## Show CPU information

```sh
lscpu | head -20
```

## Extract a tar.gz

```sh @tags=archive
tar -xzf {{file}}
```

## Extract into a specific directory

```sh @tags=archive
tar -xzf {{file}} -C {{dir}}
```

## Create a tar.gz

```sh @tags=archive
tar -czf {{out}}.tar.gz {{dir}}
```

## Create an archive excluding some directories

```sh @tags=archive
tar -czf {{out}}.tar.gz --exclude='node_modules' --exclude='.git' {{dir}}
```

## List an archive's contents without extracting

```sh @tags=archive
tar -tzf {{file}} | head -50
```

## Extract a zip

```sh @tags=archive
unzip {{file}} -d {{dir}}
```

## Make a file executable

```sh @tags=perm
chmod +x {{file}}
```

## Change directory ownership recursively

Wrong ownership after a deploy, leaving the service unable to read its files, is very common.

```sh @tags=perm @confirm
sudo chown -R {{user}}:{{user}} {{dir}}
```

## Set directories to 755 and files to 644

```sh @tags=perm @confirm
find {{dir}} -type d -exec chmod 755 {} \; && find {{dir}} -type f -exec chmod 644 {} \;
```

## Create a symlink

```sh
ln -s {{target}} {{link}}
```

## Find what is holding a file open

For when a file will not delete or a mount will not unmount.

```sh
sudo lsof {{file}}
```

## List every file a process has open

```sh
sudo lsof -p {{pid}}
```

## Call an HTTP endpoint and show the response headers

```sh @tags=network
curl -i {{url}}
```

## Show only the status code and total time

```sh @tags=network
curl -s -o /dev/null -w "code=%{http_code} time=%{time_total}s\n" {{url}}
```

## Send a JSON POST

```sh @tags=network
curl -X POST {{url}} -H "Content-Type: application/json" -d '{{json}}'
```

## Send a request with a bearer token

```sh @tags=network
curl {{url}} -H "Authorization: Bearer {{token}}"
```

## Download a file with a progress bar

```sh @tags=network
curl -L -o {{out}} {{url}}
```

## Resolve a domain name

```sh @tags=network
dig +short {{domain}}
```

## Edit the current user's cron jobs

```sh @tags=cron
crontab -e
```

## List the current user's cron jobs

```sh @tags=cron
crontab -l
```

## Common cron expressions

minute hour day month weekday.

```txt @tags=reference
*/5 * * * *     every 5 minutes
0 * * * *       every hour, on the hour
0 3 * * *       every day at 03:00
0 3 * * 1       every Monday at 03:00
0 3 1 * *       the 1st of every month at 03:00
```

## Show recent logins

```sh
last -n 20
```

## Sync the system clock

```sh @confirm
sudo timedatectl set-ntp true && timedatectl status
```
