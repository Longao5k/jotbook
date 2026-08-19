---
name: git
description: Git —— 撤销、变基、暂存、找回丢失的提交
tags: [vcs, daily]
vars:
  branch:
    desc: 分支名
    from: shell
    cmd: git branch --format=%(refname:short)
  remote_branch:
    desc: 远端分支名
    from: shell
    cmd: git branch -r --format=%(refname:lstrip=3)
  commit:
    desc: 提交
    from: shell
    cmd: git log --oneline -30
  n:
    desc: 最近几个提交
    from: ask
    options: ["1", "2", "3", "5", "10"]
---

## 撤销最后一次提交，但保留改动

改动回到暂存区，提交没了。想重新组织一次提交时最常用。

```sh @tags=undo
git reset --soft HEAD~1
```

## 撤销最后一次提交，改动退回工作区

```sh @tags=undo
git reset HEAD~1
```

## 彻底丢弃所有本地改动

不可逆。已 commit 的可以用 reflog 找回，未 commit 的找不回来。

```sh @tags=undo @confirm
git reset --hard HEAD
```

## 撤销一个已经推送的提交

生成一个反向提交，历史不变，可以安全推送。

```sh @tags=undo
git revert {{commit}}
```

## 修改最后一次提交的信息

已推送过就不要改，否则要强推。

```sh
git commit --amend -m "{{message}}"
```

## 把改动追加到上一次提交，信息不变

```sh
git add -A && git commit --amend --no-edit
```

## 找回误删的提交或分支

reflog 记录了 HEAD 的每一次移动，硬重置、删分支、变基失败都能从这里救回来。

```sh @tags=undo
git reflog
```

## 从 reflog 恢复到某个状态

```sh @tags=undo
git reset --hard {{commit}}
```

## 暂存当前改动

```sh @tags=stash
git stash push -m "{{message}}"
```

## 暂存包括未跟踪的新文件

默认的 stash 不会带上未跟踪文件，这是最常见的坑。

```sh @tags=stash
git stash push -u -m "{{message}}"
```

## 恢复最近一次暂存

```sh @tags=stash
git stash pop
```

## 查看所有暂存

```sh @tags=stash
git stash list
```

## 交互式变基最近 N 个提交

用来压缩、改序、改信息。

```sh @tags=rebase
git rebase -i HEAD~{{n}}
```

## 变基冲突解决后继续

```sh @tags=rebase
git rebase --continue
```

## 变基搞砸了，退回去

```sh @tags=rebase
git rebase --abort
```

## 把某个提交摘到当前分支

```sh
git cherry-pick {{commit}}
```

## 更安全的强制推送

如果远端有别人的新提交就会拒绝，比 --force 安全得多。变基后推送应该永远用这个。

```sh @confirm
git push --force-with-lease
```

## 推送新分支并建立跟踪关系

```sh
git push -u origin {{branch}}
```

## 删除远端分支

```sh @confirm
git push origin --delete {{remote_branch}}
```

## 拉取并清理已在远端删除的分支引用

```sh
git fetch --prune
```

## 删除所有已合并到当前分支的本地分支

```sh @platform=linux,macos @confirm
git branch --merged | grep -vE '^\*|main|master|develop' | xargs -r -n 1 git branch -d
```

## 删除所有已合并的本地分支（PowerShell）

```ps1 @platform=windows @confirm
git branch --merged | Where-Object { $_ -notmatch '^\*|main|master|develop' } | ForEach-Object { git branch -d $_.Trim() }
```

## 图形化查看所有分支的提交线

```sh
git log --oneline --graph --decorate --all
```

## 查看某个文件的完整修改历史

--follow 让它能穿过重命名。

```sh
git log --follow -p -- {{file}}
```

## 查看某人的提交

```sh
git log --author="{{author}}" --oneline --since="{{since}}"
```

## 比较两个分支差了哪些文件

```sh
git diff --name-status {{base}}..{{head}}
```

## 把单个文件恢复到某次提交的状态

```sh
git checkout {{commit}} -- {{file}}
```

## 查看某一行是谁改的

```sh
git blame -L {{start}},{{end}} -- {{file}}
```

## 删除所有未跟踪的文件和目录

清理构建产物很好用，但会删掉没 add 过的新文件。-n 换成 -f 之前先用 -n 预览。

```sh @confirm
git clean -fd
```

## 预览 clean 会删什么

```sh
git clean -nd
```

## 只改文件名大小写

Windows 和 macOS 的文件系统不区分大小写，直接 mv 时 Git 看不到变化。

```sh
git mv --force {{old}} {{new}}
```

## 打标签并推送

```sh
git tag -a {{tag}} -m "{{message}}" && git push origin {{tag}}
```

## 删除本地和远端的标签

```sh @confirm
git tag -d {{tag}} && git push origin :refs/tags/{{tag}}
```

## 查看远端地址

```sh
git remote -v
```

## 修改远端地址

```sh
git remote set-url origin {{url}}
```

## 只克隆最近一次提交

大仓库救命用。

```sh
git clone --depth 1 {{url}}
```

## 把浅克隆补全成完整历史

```sh
git fetch --unshallow
```

## 设置本仓库的用户名和邮箱

```sh
git config user.name "{{name}}" && git config user.email "{{email}}"
```

## 让 Git 忽略文件权限位的变化

Windows 与 Linux 混用时常见的噪音来源。

```sh
git config core.fileMode false
```

## 查看仓库里体积最大的文件

```sh @platform=linux,macos
git rev-list --objects --all | git cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(rest)' | awk '/^blob/ {print substr($0,6)}' | sort -k2 -nr | head -20
```
