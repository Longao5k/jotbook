---
name: jot
description: Jotbook 自己 —— 怎么记、怎么找、怎么写笔记本
tags: [meta]
---

## 打开选择器

不带参数直接运行就行。也可以在 shell 里按 Ctrl+J。

```sh @tags=daily
jot
```

## 带关键词直接搜索

```sh @tags=daily
jot {{keyword}}
```

## 把刚敲过的命令存下来

不带参数时会取 shell 历史里的最后一条。这是让笔记本长起来的主要方式。

```sh @tags=capture
jot save
```

## 存一条指定的命令

```sh @tags=capture
jot save "{{command}}"
```

## 从 shell 历史批量导入

按使用频次排序，勾选要留下的。装好之后第一件事就该做这个。

```sh @tags=capture
jot import history --top 40
```

## 列出所有条目

```sh
jot ls
```

## 只看某个笔记本

```sh
jot ls --notebook {{notebook}}
```

## 编辑自己的笔记本

用 $EDITOR 打开，没设置就用系统默认。

```sh @tags=edit
jot edit {{notebook}}
```

## 新建一个笔记本

```sh @tags=edit
jot new {{name}}
```

## 打开数据目录

所有笔记本都是这里的 .md 文件，直接用编辑器改、用 git 管都行。

```sh @tags=edit
jot path
```

## 切换 Profile

Profile 存的是你自己环境里的常量：服务名、主机、数据库名。切了之后所有 from: profile 的变量自动跟着变。

```sh @tags=profile
jot use {{profile}}
```

## 查看当前 Profile 的所有变量

```sh @tags=profile
jot profile
```

## 设置一个 Profile 变量

```sh @tags=profile
jot profile set {{key}} {{value}}
```

## 安装 shell 集成（PowerShell）

把这一行加到 $PROFILE 里，之后按 Ctrl+J 就能把命令直接填进命令行。

```ps1 @platform=windows @tags=setup
Add-Content $PROFILE "`njot init powershell | Out-String | Invoke-Expression"
```

## 安装 shell 集成（bash）

```sh @platform=linux,macos @tags=setup
echo 'eval "$(jot init bash)"' >> ~/.bashrc
```

## 安装 shell 集成（zsh）

```sh @platform=linux,macos @tags=setup
echo 'eval "$(jot init zsh)"' >> ~/.zshrc
```

## 自检

打印数据目录、笔记本数量、条目数量和索引耗时。出问题先跑这个。

```sh
jot doctor
```

## 笔记本格式速查

笔记本就是普通 Markdown。二级标题是条目名，标题下的段落是说明，代码块是命令本身。

```txt @tags=reference
## 条目标题

这里写说明，为什么用、有什么坑。

​```sh @platform=linux @confirm @tags=deploy
sudo systemctl restart {{service}}
​```

代码块属性：
  @platform=  windows / linux / macos，逗号分隔；非当前平台的条目会被隐藏
  @confirm    危险命令，注入前多一步确认
  @remote     预期在 ssh 之后使用，会禁用该条目的动态变量
  @tags=      额外的搜索关键词
```

## 变量速查

没有在 frontmatter 里声明的变量，默认就是「每次问你」。

```yaml @tags=reference
vars:
  service:
    desc: 服务名
    from: profile          # 从当前 Profile 取；Profile 里没有就退回询问
    cmd: systemctl list-units --type=service --no-legend --plain | awk '{print $1}'
  branch:
    desc: 分支
    from: shell            # 跑一条命令生成候选列表
    cmd: git branch --format=%(refname:short)
  env:
    desc: 环境
    from: ask              # 给一组固定选项
    options: ["Development", "Production"]

# 行内默认值：  {{port=8080}}
# 内置变量：    {{@cwd}} {{@clipboard}} {{@date}} {{@git.branch}} {{@env.NAME}}
# 字面大括号：  \{{ 不会被当成变量（Go 模板如 {{.Names}} 会自动识别，无需转义）
```
