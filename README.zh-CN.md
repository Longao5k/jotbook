# Jotbook

[English](README.md) · **简体中文**

[![CI](https://github.com/Longao5k/jotbook/actions/workflows/ci.yml/badge.svg)](https://github.com/Longao5k/jotbook/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

一个存**你自己**常用命令的笔记本。按一个键，命令就落到命令行上 —— 回车由你按。

```
┌ jot  4/459 ──────────────────────────────────────┐
│ › docker 日志                                    │
└──────────────────────────────────────────────────┘
❯ 跟踪容器日志                              docker
    docker logs -f --tail 200 ⟨container⟩
  查看容器最近的日志并带时间戳              docker
    docker logs --timestamps --since 30m ⟨container⟩
───────────────────────────────────────────────────
 ↑↓ 选择   ⏎ 使用   ^E 打开文件   esc 取消
```

选中、填空，完整的命令就在你的命令行上等着：

```
$ docker logs -f --tail 200 kestrel-api▏
```

## 为什么

不是「我不知道这条命令怎么写」—— 那个问题 AI 已经解决了。

是「我知道，我上周还用过，但我不想再翻一遍聊天记录」。这是**检索**问题，不是生成问题。
而 `sudo systemctl restart kestrel-orders-api.service` 这种**你自己机器上**的精确命令，
本来也不该交给一个非确定性的模型每次重新生成一遍。

## 特点

- **中英双语** —— 界面和内置笔记本一起切换，`jot lang` 一条命令搞定
- **开箱不是空的** —— 内置 19 个笔记本、630+ 条命令：
  git · linux · macos · powershell · ssh · tmux · docker · kubectl · nginx · systemd · dotnet · flutter · npm · python · mssql · mysql · postgres · redis
- **越用越顺手** —— 常用和最近用过的条目自动往上浮。搜索框空着时，列表就是你最常用的那几条
- **变量** —— `{{service}}` 可以来自你的 Profile、来自一条实时命令的输出，或者每次问你
- **只注入，不执行** —— jot 把命令放到你的命令行上就结束了，回车永远由人按
- **纯文本** —— 全是 Markdown 文件，用 git 同步、在 GitHub 上直接看、拿 vim 改都行
- **无账号、无服务器、无遥测**
- **Windows 一等公民** —— 自带模糊搜索和界面，不依赖 fzf

## 安装

去 [Releases](https://github.com/Longao5k/jotbook/releases) 下一个二进制 —— Windows、Linux、Mac 两种架构都有 —— 把 `jot` 放进 `PATH` 就行。

或者自己编译，需要 [Rust](https://rustup.rs)：

```bash
cargo install --git https://github.com/Longao5k/jotbook jot-cli
```

然后接上你的 shell。只有一行，脚本本体随二进制发布，以后升级不用再动配置。

<details open>
<summary><b>PowerShell</b></summary>

```powershell
Add-Content $PROFILE "`njot init powershell | Out-String | Invoke-Expression"
```
</details>

<details>
<summary><b>bash</b></summary>

```bash
echo 'eval "$(jot init bash)"' >> ~/.bashrc
```
</details>

<details>
<summary><b>zsh</b></summary>

```bash
echo 'eval "$(jot init zsh)"' >> ~/.zshrc
```
</details>

<details>
<summary><b>fish</b></summary>

```bash
echo 'jot init fish | source' >> ~/.config/fish/config.fish
```
</details>

重开终端，然后按 **<kbd>Ctrl</kbd>+<kbd>J</kbd>**（PowerShell）或 **<kbd>Ctrl</kbd>+<kbd>G</kbd>**（bash / zsh / fish）。

> 两边不一样是故意的：`Ctrl`+`J` 在终端里就是 LF，readline 会当成回车，绑它会毁掉换行；PSReadLine 收到的是独立按键事件，没这个问题。想改用 `jot init bash --key '\C-o'`。

## 上手

```bash
jot import history --top 40                              # 从 shell 历史按使用频次导入
jot profile set service kestrel-orders-api.service   # 配好你自己环境里的常量
jot                                                      # 开始用
```

## 命令一览

| 命令 | 作用 |
|---|---|
| `jot` | 打开选择器 |
| `jot docker 日志` | 带搜索词直接打开 |
| `jot save` | 把刚敲过的那条命令存下来 |
| `jot save "<命令>"` | 存一条指定的命令 |
| `jot import history` | 从 shell 历史批量导入，按频次排序 |
| `jot import text` | 粘贴批量导入 —— 默认读剪贴板 |
| `jot import text -n <名字>` | 导入到指定笔记本 |
| `jot ls` | 列出全部条目 |
| `jot notebooks` | 列出所有笔记本，以及在选择器里用的 `@名字` |
| `jot edit [笔记本]` | 用 `$EDITOR` 打开 |
| `jot new <名字>` | 新建笔记本 |
| `jot use <profile>` | 切换 Profile |
| `jot profile set <键> <值>` | 设置 Profile 变量 |
| `jot add gh:user/repo` | 装一个社区笔记本源（git 仓库）|
| `jot sources` | 列出已装的源 |
| `jot sync [名字]` | 更新源 |
| `jot trust <名字>` | 授信，允许该源的 `from: shell` 变量真的执行 |
| `jot remove <名字>` | 卸载一个源 |
| `jot lang [en\|zh\|auto]` | 查看或设置界面与笔记本语言 |
| `jot doctor` | 自检 |
| `jot path` | 打印数据目录 |
| `jot pick -q "<词>" --first` | 不开界面，最佳匹配直接打到 stdout（脚本用） |


### 按键

| 键 | |
|---|---|
| <kbd>↑</kbd> <kbd>↓</kbd> | 上下移动 |
| <kbd>Enter</kbd> | 使用选中的命令 |
| <kbd>Ctrl</kbd>+<kbd>B</kbd> | 填变量时回到上一步 |
| <kbd>Esc</kbd> | 直接退出，在哪一步都一样 |
| <kbd>Ctrl</kbd>+<kbd>E</kbd> | 用 `$EDITOR` 打开这条命令 |
| `@笔记本` `#标签` | 在搜索框里打，用来缩小范围 |

## 怎么写笔记本

笔记本就是普通 Markdown。`##` 标题是条目名，标题下的段落是说明，代码块是命令本身。

````markdown
---
name: my-servers
description: 我的服务器
vars:
  service:
    desc: systemd 服务名
    from: profile          # Profile 里有就直接用，没有就退回询问
    cmd: systemctl list-units --type=service --no-legend --plain | awk '{print $1}'
---

## 重启后端服务

改完 appsettings 之后必须重启，改 nginx 配置不用。

```sh @platform=linux @confirm @tags=deploy
sudo systemctl restart {{service}}
```
````

### 代码块属性

| 属性 | 作用 |
|---|---|
| `@platform=` | `windows` / `linux` / `macos`，逗号分隔。非当前平台的条目自动隐藏 |
| `@confirm` | 危险命令。注入前多一步确认，列表里带 ⚠ |
| `@remote` | 预期在 `ssh` 之后使用。会禁用该条目的动态变量 —— 它们会在**本地**求值 |
| `@tags=` | 额外的搜索关键词 |

### 在选择器里缩小范围

六百条一条条翻太累。两个前缀可以限定范围：

| 输入 | 结果 |
|---|---|
| `@docker` | 只看 docker 这一本 |
| `#deploy` | 只看打了 deploy 标签的 |
| `@docker 日志` | 范围和搜索词一起用 |

`jot notebooks` 列出所有可用的名字。所有条件都要满足，所以 `@git #undo reset` 是三重收窄。

### 变量的三层

| 来源 | 行为 |
|---|---|
| `ask`（默认） | 每次问你。配 `options` 可以给固定候选 |
| `profile` | 从当前 Profile 直接取，不打断流程；没配就自动降级 |
| `shell` | 跑 `cmd`，把输出变成可模糊搜索的候选列表 |

**没有声明**的 `{{name}}` 同样会先查当前 Profile。这是 `jot save` 的自动参数化能闭环的关键 —— 它把值改写成 `{{service}}` 时并不会附带声明，不查 Profile 的话生成出来的条目反而用不了。想每次都被问，就显式写 `from: ask`。

内置变量：`{{@cwd}}` `{{@date}}` `{{@host}}` `{{@git.branch}}` `{{@git.root}}` `{{@env.NAME}}`
行内默认值：`{{port=8080}}`

**Go 模板原样可用。** `{{ }}` 和 Docker、kubectl 撞车，所以 jot 只把**纯标识符**当变量：

```sh
docker ps --format "{{.Names}}\t{{.Status}}"       # 字面量，不用转义
kubectl get x -o go-template='{{range .items}}…'   # 字面量
sudo systemctl restart {{service}}                  # 这个才是变量
```

真要一个字面的 `{{name}}`，写成 `\{{name}}`。

## 社区笔记本

一个源就是一个 git 仓库。jot 直接调 `git`，所以私有仓库、SSH 密钥、增量更新全都不用额外配置。

```bash
jot add gh:someone/jot-notebooks
```

jot 会先看 `notebooks/` 子目录，没有就看仓库根的 `*.md`（README、LICENSE 这类会跳过）。源放在 `~/.jot/notebooks/sources/<名字>/`，你也可以自己 `git clone` 进这个目录。

**外部源默认不可信。** 笔记本里的 `from: shell` 变量只是为了**生成一个候选列表**，就会在你机器上执行任意命令 —— 光是浏览一个恶意笔记本就足够中招。所以 `builtin/` 和 `local/` 以外的一切，动态变量都是关的，直到你看过内容并显式打开：

```bash
jot trust someone-jot-notebooks
```

未授信不影响其它功能。而且命令本身依然只是被**打到你的命令行上**，从不执行。

## 语言

jot 说中英两种语言。界面和内置笔记本一起切换 —— 条目的标题和说明也会跟着变成你读得懂的那种。

```bash
jot lang zh
```

优先级：显式的 `jot lang` 设置 → `JOT_LANG` 或常规 locale 变量 → 操作系统语言 → 英文。
`jot lang auto` 把控制权交回环境变量。

**两种语言里命令本身完全相同**，只有说明文字不同，这一点有测试强制保证。
仓库里笔记本放在 `notebooks/en/` 和 `notebooks/zh/`，落地到 `~/.jot/notebooks/builtin/` 的只有当前语言那一套。

## 数据放在哪

```
~/.jot/
├── notebooks/
│   ├── builtin/     随二进制发布的当前语言那一套，升级时会被重写
│   ├── local/       你自己的，永远不会被动
│   └── sources/     社区源（git 克隆），默认不可信
├── config.toml      当前 Profile
├── profiles.toml    你环境里的常量 —— 不要放密钥
└── usage.toml       各条目的使用次数，用来排序
```

`notebooks/` 就是个普通目录，`git init` 一下就有了同步、历史和分享。`profiles.toml` 刻意留在外面。

## 参与贡献

**笔记本是最有价值的贡献。** 给 `notebooks/zh/*.md` 加命令的 PR 完全不需要写 Rust，按上面的格式写 Markdown 就行。有余力的话把对应的 `notebooks/en/*.md` 也补上 —— 有测试检查两种语言的结构必须完全一致。比起把 `--help` 抄一遍，那些**人真的记不住**的命令价值高得多。

要改客户端本身的话，动大工程之前先看 [docs/design.html](docs/design.html)，里面有架构、关键决策记录和非目标。

### 构建

```bash
cargo test
cargo build --release
```

Windows 上请用 MSVC 工具链 —— GNU host 需要完整的 MinGW，因为 `dlltool` 会去调 `as`：

```bash
rustup default stable-x86_64-pc-windows-msvc
```

## 许可

MIT
