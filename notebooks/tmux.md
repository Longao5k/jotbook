---
name: tmux
description: tmux —— 会话保活、窗格、复制模式
tags: [ops, terminal]
platform: [linux, macos]
vars:
  session:
    desc: 会话名
    from: shell
    cmd: tmux list-sessions -F "#{session_name}" 2>/dev/null
---

## 新建一个命名会话

ssh 上去跑长任务前先开一个，断线了任务不会死。

```sh @tags=daily
tmux new -s {{session}}
```

## 重新接上已有会话

断线重连之后最常用的一条。

```sh @tags=daily
tmux attach -t {{session}}
```

## 接上最近一个会话

```sh @tags=daily
tmux attach
```

## 列出所有会话

```sh @tags=daily
tmux ls
```

## 结束一个会话

```sh @confirm
tmux kill-session -t {{session}}
```

## 结束除当前外的所有会话

```sh @confirm
tmux kill-session -a
```

## 在后台新建会话并直接跑一条命令

不进入会话，适合脚本里用。

```sh
tmux new -d -s {{session}} '{{command}}'
```

## 在已有会话里发一条命令

```sh
tmux send-keys -t {{session}} '{{command}}' Enter
```

## 快捷键速查

默认前缀是 <kbd>Ctrl</kbd>+<kbd>b</kbd>。下面都是「先按前缀，再按后面的键」。

```txt @tags=reference
会话
  d          脱离当前会话（任务继续在后台跑）
  s          列出会话并切换
  $          重命名当前会话

窗口（tab）
  c          新建窗口
  n / p      下一个 / 上一个窗口
  0-9        跳到第 N 个窗口
  ,          重命名窗口
  &          关闭窗口
  w          列出所有窗口

窗格（split）
  %          左右分屏
  "          上下分屏
  方向键      在窗格间移动
  z          当前窗格全屏 / 还原（非常好用）
  x          关闭当前窗格
  空格        切换窗格布局
  Ctrl+方向键 调整窗格大小

复制
  [          进入复制模式（然后可以用方向键 / PgUp 翻历史）
  空格        开始选择
  回车        复制并退出
  ]          粘贴
  q          退出复制模式

其它
  ?          显示所有快捷键
  :          进入命令行模式
```

## 左右分屏

```sh
tmux split-window -h
```

## 上下分屏

```sh
tmux split-window -v
```

## 开启鼠标支持

能用鼠标点窗格、滚轮翻历史。新手强烈建议开。

```sh
tmux set -g mouse on
```

## 让鼠标支持永久生效

```sh
echo 'set -g mouse on' >> ~/.tmux.conf && tmux source-file ~/.tmux.conf
```

## 加大历史回滚行数

默认只有 2000 行，跑构建时根本不够。

```sh
echo 'set -g history-limit 50000' >> ~/.tmux.conf && tmux source-file ~/.tmux.conf
```

## 重新加载配置

```sh
tmux source-file ~/.tmux.conf
```

## 把前缀键改成 Ctrl+a

Ctrl+b 和 Vim 的翻页冲突，很多人改成 Ctrl+a。

```sh @tags=reference
cat >> ~/.tmux.conf <<'EOF'
unbind C-b
set -g prefix C-a
bind C-a send-prefix
EOF
tmux source-file ~/.tmux.conf
```

## 把当前窗格的历史存成文件

跑完构建想把完整输出留下来时用。

```sh
tmux capture-pane -pS -50000 > {{file}}
```

## 一条命令搭好开发环境

新建会话、分屏、各自跑不同命令。写进脚本每天一键开工。

```sh @tags=reference
tmux new -d -s {{session}} -c ~/project
tmux send-keys -t {{session}} 'dotnet watch run' Enter
tmux split-window -t {{session}} -h -c ~/project
tmux send-keys -t {{session}} 'npm run dev' Enter
tmux split-window -t {{session}} -v -c ~/project
tmux send-keys -t {{session}} 'docker compose logs -f' Enter
tmux attach -t {{session}}
```

## 让会话开机自启

```sh @tags=reference
# crontab -e 里加：
@reboot /usr/bin/tmux new -d -s {{session}} '{{command}}'
```
