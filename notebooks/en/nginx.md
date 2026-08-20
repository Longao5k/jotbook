---
name: nginx
description: nginx - validating config, graceful reloads, logs, reverse proxy debugging
tags: [ops, web]
platform: [linux, macos]
vars:
  site:
    desc: site config
    from: shell
    cmd: ls /etc/nginx/sites-available 2>/dev/null || ls /etc/nginx/conf.d 2>/dev/null
---

## Validate the configuration

**Always run this after editing config.** Reloading with a syntax error leaves nginx unable to start.

```sh @tags=daily
sudo nginx -t
```

## Reload configuration gracefully

Keeps connections open and lets in-flight requests finish. Use this for everyday config changes, not restart.

```sh @tags=daily
sudo nginx -t && sudo systemctl reload nginx
```

## Restart (drops connections)

Only needed for startup parameters such as worker count or listen ports.

```sh @confirm
sudo systemctl restart nginx
```

## Show the service status

```sh
systemctl status nginx
```

## Show which modules were compiled in

For when a directive reports "unknown directive".

```sh
nginx -V
```

## Show the fully merged configuration

What it looks like with every include expanded, which settles which block actually applies.

```sh
sudo nginx -T
```

## Follow the access log

```sh @tags=logs @remote
sudo tail -f /var/log/nginx/access.log
```

## Follow the error log

Start here for 502 and 504.

```sh @tags=logs @remote
sudo tail -f /var/log/nginx/error.log
```

## Show only requests with a given status code

```sh @tags=logs
sudo awk '$9 == {{code}}' /var/log/nginx/access.log | tail -50
```

## Count requests by status code

```sh @tags=logs
sudo awk '{print $9}' /var/log/nginx/access.log | sort | uniq -c | sort -rn
```

## The most requested URLs

```sh @tags=logs
sudo awk '{print $7}' /var/log/nginx/access.log | sort | uniq -c | sort -rn | head -20
```

## The busiest client IPs

For spotting someone hammering an endpoint.

```sh @tags=logs
sudo awk '{print $1}' /var/log/nginx/access.log | sort | uniq -c | sort -rn | head -20
```

## Find the slowest requests

Requires $request_time in the log format.

```sh @tags=logs
sudo awk '{print $NF, $7}' /var/log/nginx/access.log | sort -rn | head -20
```

## Enable a site (Debian/Ubuntu layout)

```sh
sudo ln -s /etc/nginx/sites-available/{{site}} /etc/nginx/sites-enabled/ && sudo nginx -t && sudo systemctl reload nginx
```

## Disable a site

```sh @confirm
sudo rm /etc/nginx/sites-enabled/{{site}} && sudo nginx -t && sudo systemctl reload nginx
```

## Edit a site's configuration

```sh
sudo nano /etc/nginx/sites-available/{{site}}
```

## A minimal reverse proxy to a local service

The block you reach for most when putting a .NET or Node service behind nginx. Without those
proxy_set_header lines the backend sees the wrong client IP and the wrong scheme.

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

        # WebSocket / SignalR needs these two lines
        proxy_set_header   Upgrade    $http_upgrade;
        proxy_set_header   Connection "upgrade";

        proxy_read_timeout 300s;
    }
}
```

## A minimal static site

```nginx @tags=reference
server {
    listen 80;
    server_name {{domain}};
    root /var/www/{{domain}};
    index index.html;

    # A missing line here is why frontend routes (Vue/React/Flutter Web) 404 on refresh
    location / {
        try_files $uri $uri/ /index.html;
    }

    location ~* \.(js|css|png|jpg|svg|woff2)$ {
        expires 30d;
        add_header Cache-Control "public, immutable";
    }
}
```

## Get and install an HTTPS certificate with certbot

```sh @confirm
sudo certbot --nginx -d {{domain}}
```

## Test automatic renewal

```sh
sudo certbot renew --dry-run
```

## Check a certificate's expiry date

```sh
echo | openssl s_client -servername {{domain}} -connect {{domain}}:443 2>/dev/null | openssl x509 -noout -dates
```

## Show nginx connection counts

Requires the stub_status module.

```sh @tags=diagnose
curl -s http://127.0.0.1/nginx_status
```

## Show the nginx worker processes

```sh @tags=diagnose
ps aux | grep '[n]ginx'
```

## Find what is holding 80 and 443

```sh @tags=diagnose
sudo ss -lntp | grep -E ':(80|443) '
```

## Empty a log file without restarting

rm on a log file does not make nginx recreate it; the handle still points at the deleted inode.

```sh @confirm
sudo truncate -s 0 /var/log/nginx/access.log
```

## Rate-limit requests per IP

```nginx @tags=reference
# in the http block
limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;

# in the location block
limit_req zone=api burst=20 nodelay;
```
