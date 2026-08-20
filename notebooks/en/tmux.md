---
name: tmux
description: tmux - keeping sessions alive, panes, copy mode
tags: [ops, terminal]
platform: [linux, macos]
vars:
  session:
    desc: session name
    from: shell
    cmd: tmux list-sessions -F "#{session_name}" 2>/dev/null
---

## Start a named session

Open one before starting a long job over ssh, and the job survives a dropped connection.

```sh @tags=daily
tmux new -s {{session}}
```

## Reattach to a session

The command you reach for after reconnecting.

```sh @tags=daily
tmux attach -t {{session}}
```

## Attach to the most recent session

```sh @tags=daily
tmux attach
```

## List sessions

```sh @tags=daily
tmux ls
```

## Kill a session

```sh @confirm
tmux kill-session -t {{session}}
```

## Kill every session except the current one

```sh @confirm
tmux kill-session -a
```

## Start a background session running one command

Does not attach, so it works well from scripts.

```sh
tmux new -d -s {{session}} '{{command}}'
```

## Send a command into an existing session

```sh
tmux send-keys -t {{session}} '{{command}}' Enter
```

## Key binding reference

The default prefix is <kbd>Ctrl</kbd>+<kbd>b</kbd>. Everything below means "press the prefix, then the key".

```txt @tags=reference
Sessions
  d          detach (the job keeps running in the background)
  s          list sessions and switch
  $          rename the current session

Windows (tabs)
  c          new window
  n / p      next / previous window
  0-9        jump to window N
  ,          rename the window
  &          close the window
  w          list all windows

Panes (splits)
  %          split left / right
  "          split top / bottom
  arrows     move between panes
  z          zoom the current pane in / out (very useful)
  x          close the current pane
  space      cycle pane layouts
  Ctrl+arrow resize the pane

Copying
  [          enter copy mode (then arrows / PgUp scroll back)
  space      start the selection
  enter      copy and exit
  ]          paste
  q          leave copy mode

Other
  ?          show every binding
  :          command prompt
```

## Split left and right

```sh
tmux split-window -h
```

## Split top and bottom

```sh
tmux split-window -v
```

## Enable mouse support

Click panes and scroll history with the wheel. Strongly recommended if you are new to tmux.

```sh
tmux set -g mouse on
```

## Make mouse support permanent

```sh
echo 'set -g mouse on' >> ~/.tmux.conf && tmux source-file ~/.tmux.conf
```

## Increase the scrollback limit

The default 2000 lines is nowhere near enough while running a build.

```sh
echo 'set -g history-limit 50000' >> ~/.tmux.conf && tmux source-file ~/.tmux.conf
```

## Reload the configuration

```sh
tmux source-file ~/.tmux.conf
```

## Change the prefix to Ctrl+a

Ctrl+b clashes with Vim's page-up, so many people move it to Ctrl+a.

```sh @tags=reference
cat >> ~/.tmux.conf <<'EOF'
unbind C-b
set -g prefix C-a
bind C-a send-prefix
EOF
tmux source-file ~/.tmux.conf
```

## Dump the current pane's history to a file

For keeping the full output of a build you just ran.

```sh
tmux capture-pane -pS -50000 > {{file}}
```

## Set up a whole dev environment in one go

Create a session, split it, and run something different in each pane. Put it in a script and start your day with one command.

```sh @tags=reference
tmux new -d -s {{session}} -c ~/project
tmux send-keys -t {{session}} 'dotnet watch run' Enter
tmux split-window -t {{session}} -h -c ~/project
tmux send-keys -t {{session}} 'npm run dev' Enter
tmux split-window -t {{session}} -v -c ~/project
tmux send-keys -t {{session}} 'docker compose logs -f' Enter
tmux attach -t {{session}}
```

## Start a session at boot

```sh @tags=reference
# in crontab -e, add:
@reboot /usr/bin/tmux new -d -s {{session}} '{{command}}'
```
