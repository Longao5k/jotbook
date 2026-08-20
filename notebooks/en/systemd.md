---
name: systemd
description: systemd - managing services and reading the journal
tags: [ops, linux]
platform: [linux]
vars:
  service:
    desc: service name
    from: profile
    cmd: systemctl list-units --type=service --no-legend --plain --no-pager | awk '{print $1}'
  since:
    desc: starting from
    from: ask
    options: ["10 min ago", "1 hour ago", "today", "yesterday"]
---

## Restart a service

The one you reach for after every config change.

```sh @confirm @tags=daily
sudo systemctl restart {{service}}
```

## Show a service's status

Includes the last ten log lines, and is the first step when a service fails to start.

```sh @tags=daily
systemctl status {{service}}
```

## Start a service

```sh
sudo systemctl start {{service}}
```

## Stop a service

```sh @confirm
sudo systemctl stop {{service}}
```

## Reload configuration without dropping connections

Only works if the service implements reload. nginx does; most .NET and Node services do not.

```sh
sudo systemctl reload {{service}}
```

## Reload if supported, otherwise restart

```sh @confirm
sudo systemctl reload-or-restart {{service}}
```

## Enable at boot and start now

```sh
sudo systemctl enable --now {{service}}
```

## Disable at boot and stop now

```sh @confirm
sudo systemctl disable --now {{service}}
```

## Reload after editing a unit file

Without this, systemd keeps using the old copy of /etc/systemd/system/*.service.

```sh @tags=daily
sudo systemctl daemon-reload
```

## Follow a service's logs

```sh @tags=logs @remote
journalctl -u {{service}} -f
```

## Show a service's recent logs

```sh @tags=logs @remote
journalctl -u {{service}} --since "{{since}}" --no-pager
```

## Show only error level and above

```sh @tags=logs @remote
journalctl -u {{service}} -p err --no-pager -n 100
```

## Show logs since this boot

```sh @tags=logs @remote
journalctl -u {{service}} -b --no-pager
```

## Show logs from the previous boot

For working out why a machine rebooted unexpectedly.

```sh @tags=logs @remote
journalctl -b -1 -p err --no-pager
```

## Emit the journal as JSON

```sh @tags=logs
journalctl -u {{service}} -o json-pretty -n 20
```

## Edit a unit file

Prompts for a daemon-reload automatically after saving.

```sh
sudo systemctl edit --full {{service}}
```

## Override part of a unit without editing the original

Creates override.conf, which survives package upgrades and is safer than editing the shipped file.

```sh
sudo systemctl edit {{service}}
```

## Show a unit file's full contents

```sh
systemctl cat {{service}}
```

## Show every effective setting for a service

```sh
systemctl show {{service}}
```

## List all services

```sh
systemctl list-units --type=service --no-pager
```

## List services that failed to start

```sh @tags=daily
systemctl --failed
```

## List services enabled at boot

```sh
systemctl list-unit-files --type=service --state=enabled --no-pager
```

## Check whether a service is running

For scripts: prints only active or inactive.

```sh
systemctl is-active {{service}}
```

## Show what took longest at boot

```sh
systemd-analyze blame | head -20
```

## See how much disk the journal is using

```sh
journalctl --disk-usage
```

## Delete logs older than 7 days

The command that saves you when logs have filled the server's disk.

```sh @confirm
sudo journalctl --vacuum-time=7d
```

## Cap the journal at 500M

```sh @confirm
sudo journalctl --vacuum-size=500M
```

## Find which service is holding a port

```sh
sudo ss -lntp | grep :{{port}}
```

## Check whether a service comes back after a reboot

```sh
systemctl is-enabled {{service}}
```

## A minimal working unit file

Drop it in /etc/systemd/system/{{service}}, then daemon-reload and enable --now.

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
