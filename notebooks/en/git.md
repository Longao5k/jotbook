---
name: git
description: Git - undoing, rebasing, stashing, recovering lost commits
tags: [vcs, daily]
vars:
  branch:
    desc: branch
    from: shell
    cmd: git branch --format=%(refname:short)
  remote_branch:
    desc: remote branch
    from: shell
    cmd: git branch -r --format=%(refname:lstrip=3)
  commit:
    desc: commit
    from: shell
    cmd: git log --oneline -30
  n:
    desc: how many recent commits
    from: ask
    options: ["1", "2", "3", "5", "10"]
---

## Undo the last commit but keep the changes

The changes go back to the index and the commit is gone. The usual move when you want to reorganise a commit.

```sh @tags=undo
git reset --soft HEAD~1
```

## Undo the last commit, changes back to the working tree

```sh @tags=undo
git reset HEAD~1
```

## Throw away every local change

Not reversible. Anything committed can be recovered through reflog; anything uncommitted cannot.

```sh @tags=undo @confirm
git reset --hard HEAD
```

## Undo a commit that is already pushed

Creates an inverse commit, leaves history intact, and is safe to push.

```sh @tags=undo
git revert {{commit}}
```

## Reword the last commit

Do not do this once it is pushed, or you will need a force push.

```sh
git commit --amend -m "{{message}}"
```

## Fold changes into the last commit, keeping its message

```sh
git add -A && git commit --amend --no-edit
```

## Recover a deleted commit or branch

reflog records every move of HEAD, so a hard reset, a deleted branch or a botched rebase can all be rescued from here.

```sh @tags=undo
git reflog
```

## Restore to a state found in the reflog

```sh @tags=undo
git reset --hard {{commit}}
```

## Stash the current changes

```sh @tags=stash
git stash push -m "{{message}}"
```

## Stash including new untracked files

A plain stash leaves untracked files behind, which is the classic trap.

```sh @tags=stash
git stash push -u -m "{{message}}"
```

## Restore the most recent stash

```sh @tags=stash
git stash pop
```

## List every stash

```sh @tags=stash
git stash list
```

## Interactively rebase the last N commits

For squashing, reordering and rewording.

```sh @tags=rebase
git rebase -i HEAD~{{n}}
```

## Continue after resolving a rebase conflict

```sh @tags=rebase
git rebase --continue
```

## Abandon a rebase that went wrong

```sh @tags=rebase
git rebase --abort
```

## Cherry-pick a commit onto the current branch

```sh
git cherry-pick {{commit}}
```

## A safer force push

Refuses if the remote has commits you have not seen, which makes it far safer than --force. Always use this after a rebase.

```sh @confirm
git push --force-with-lease
```

## Push a new branch and set up tracking

```sh
git push -u origin {{branch}}
```

## Delete a remote branch

```sh @confirm
git push origin --delete {{remote_branch}}
```

## Fetch and prune references to branches deleted upstream

```sh
git fetch --prune
```

## Delete every local branch already merged into this one

```sh @platform=linux,macos @confirm
git branch --merged | grep -vE '^\*|main|master|develop' | xargs -r -n 1 git branch -d
```

## Delete every merged local branch (PowerShell)

```ps1 @platform=windows @confirm
git branch --merged | Where-Object { $_ -notmatch '^\*|main|master|develop' } | ForEach-Object { git branch -d $_.Trim() }
```

## Draw the commit graph across all branches

```sh
git log --oneline --graph --decorate --all
```

## Show the full history of one file

--follow lets it trace through renames.

```sh
git log --follow -p -- {{file}}
```

## Show one person's commits

```sh
git log --author="{{author}}" --oneline --since="{{since}}"
```

## See which files differ between two branches

```sh
git diff --name-status {{base}}..{{head}}
```

## Restore a single file to how it looked in a commit

```sh
git checkout {{commit}} -- {{file}}
```

## Find out who changed a particular line

```sh
git blame -L {{start}},{{end}} -- {{file}}
```

## Delete every untracked file and directory

Great for clearing build output, but it also deletes new files you never added. Preview with -n before switching to -f.

```sh @confirm
git clean -fd
```

## Preview what clean would delete

```sh
git clean -nd
```

## Change only the case of a filename

Windows and macOS filesystems are case-insensitive, so a plain mv leaves Git seeing no change at all.

```sh
git mv --force {{old}} {{new}}
```

## Create a tag and push it

```sh
git tag -a {{tag}} -m "{{message}}" && git push origin {{tag}}
```

## Delete a tag locally and on the remote

```sh @confirm
git tag -d {{tag}} && git push origin :refs/tags/{{tag}}
```

## Show the remote URL

```sh
git remote -v
```

## Change the remote URL

```sh
git remote set-url origin {{url}}
```

## Clone only the most recent commit

A lifesaver on large repositories.

```sh
git clone --depth 1 {{url}}
```

## Turn a shallow clone into a full one

```sh
git fetch --unshallow
```

## Set the name and email for this repository only

```sh
git config user.name "{{name}}" && git config user.email "{{email}}"
```

## Make Git ignore file mode changes

A common source of noise when mixing Windows and Linux.

```sh
git config core.fileMode false
```

## Find the largest files in the repository

```sh @platform=linux,macos
git rev-list --objects --all | git cat-file --batch-check='%(objecttype) %(objectname) %(objectsize) %(rest)' | awk '/^blob/ {print substr($0,6)}' | sort -k2 -nr | head -20
```
