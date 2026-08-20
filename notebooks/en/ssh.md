---
name: ssh
description: SSH - logging in, keys, port forwarding, syncing files
tags: [ops, network]
vars:
  host:
    desc: host
    from: profile
    cmd: grep -i "^Host " ~/.ssh/config | awk '{print $2}' | grep -v "[*?]"
---

## Log in

```sh @tags=daily
ssh {{host}}
```

## Log in as a specific user on a specific port

```sh
ssh -p {{port}} {{user}}@{{host}}
```

## Generate an ed25519 key

Use ed25519 rather than rsa these days: shorter, faster and safer.

```sh @tags=key
ssh-keygen -t ed25519 -C "{{comment}}"
```

## Install your public key on a server

```sh @platform=linux,macos @tags=key
ssh-copy-id {{user}}@{{host}}
```

## Install your public key on a server (Windows)

Windows has no ssh-copy-id, so use this instead.

```ps1 @platform=windows @tags=key
type "$env:USERPROFILE\.ssh\id_ed25519.pub" | ssh {{user}}@{{host}} "mkdir -p ~/.ssh && cat >> ~/.ssh/authorized_keys"
```

## Show your public key

```ps1 @platform=windows @tags=key
Get-Content "$env:USERPROFILE\.ssh\id_ed25519.pub"
```

## Show your public key (bash)

```sh @platform=linux,macos @tags=key
cat ~/.ssh/id_ed25519.pub
```

## Log in with a specific key file

```sh @tags=key
ssh -i {{keyfile}} {{user}}@{{host}}
```

## The server was rebuilt and the host key no longer matches

Drop the old record and reconnect. If you did not rebuild it yourself, treat it as a possible man in the middle.

```sh
ssh-keygen -R {{host}}
```

## Test the connection without logging in

```sh
ssh -T {{user}}@{{host}}
```

## Debug a connection that will not open

Prints the whole handshake, which is where to look when authentication fails.

```sh
ssh -vvv {{host}}
```

## Local forward: bring a remote service to your machine

For reaching a database or admin panel only exposed inside the server's network. Then open localhost:<local port> in a browser.

```sh @tags=tunnel
ssh -N -L {{localport}}:localhost:{{remoteport}} {{host}}
```

## Local forward to a third host

A machine the server can reach but you cannot.

```sh @tags=tunnel
ssh -N -L {{localport}}:{{target}}:{{remoteport}} {{host}}
```

## Remote forward: expose a local service to the server

Lets the server call a service on your dev machine, which is excellent for debugging webhooks.

```sh @tags=tunnel
ssh -N -R {{remoteport}}:localhost:{{localport}} {{host}}
```

## Open a SOCKS5 proxy

Point a browser at socks5://127.0.0.1:1080 and you are using the server's network egress.

```sh @tags=tunnel
ssh -N -D 1080 {{host}}
```

## Run a tunnel in the background

```sh @tags=tunnel
ssh -f -N -L {{localport}}:localhost:{{remoteport}} {{host}}
```

## Upload one file

```sh @tags=copy
scp {{file}} {{host}}:{{dest}}
```

## Download one file

```sh @tags=copy
scp {{host}}:{{src}} .
```

## Upload a whole directory

```sh @tags=copy
scp -r {{dir}} {{host}}:{{dest}}
```

## Sync a directory with rsync, transferring only the differences

Much faster than scp on large directories, and it resumes after an interruption.

```sh @tags=copy
rsync -avz --progress {{src}} {{host}}:{{dest}}
```

## rsync while excluding node_modules and friends

```sh @tags=copy
rsync -avz --progress --exclude 'node_modules' --exclude '.git' --exclude 'bin' --exclude 'obj' {{src}} {{host}}:{{dest}}
```

## rsync and delete anything extra on the far side

Makes the target match the source exactly. It deletes files, so preview with --dry-run first.

```sh @tags=copy @confirm
rsync -avz --delete {{src}} {{host}}:{{dest}}
```

## Preview what rsync would do

```sh @tags=copy
rsync -avzn --delete {{src}} {{host}}:{{dest}}
```

## Run one command on the remote and return

```sh
ssh {{host}} "{{command}}"
```

## Run a command on the remote under sudo

```sh @confirm
ssh -t {{host}} "sudo {{command}}"
```

## Pipe a local file's contents to a remote file

```sh
cat {{file}} | ssh {{host}} "cat > {{dest}}"
```

## A useful ssh config template

Put it in ~/.ssh/config and `ssh prod` just works. ServerAliveInterval stops an idle session being dropped.

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

## Fix the "permissions are too open" error on a private key

On Linux and macOS a private key must be 600 or ssh refuses to use it.

```sh @platform=linux,macos @tags=key
chmod 600 ~/.ssh/id_ed25519 && chmod 700 ~/.ssh
```

## Start ssh-agent and load a key (PowerShell)

```ps1 @platform=windows @tags=key
Start-Service ssh-agent
ssh-add "$env:USERPROFILE\.ssh\id_ed25519"
```

## Start ssh-agent and load a key (bash)

```sh @platform=linux,macos @tags=key
eval "$(ssh-agent -s)" && ssh-add ~/.ssh/id_ed25519
```
