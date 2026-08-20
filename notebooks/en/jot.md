---
name: jot
description: Jotbook itself - how to save, find and write notebooks
tags: [meta]
vars:
  notebook:
    desc: notebook
    from: shell
    cmd: jot notebooks --names
---

## Open the picker

Just run it with no arguments. Ctrl+J does the same from your shell.

```sh @tags=daily
jot
```

## Search with a term straight away

```sh @tags=daily
jot {{keyword}}
```

## Save the command you just ran

With no argument it takes the last entry from your shell history. This is the main way a notebook grows.

```sh @tags=capture
jot save
```

## Save a specific command

```sh @tags=capture
jot save "{{command}}"
```

## Bulk import from shell history

Ranked by how often you use them; tick the ones worth keeping. Do this first after installing.

```sh @tags=capture
jot import history --top 40
```

## List every entry

```sh
jot ls
```

## List one notebook only

```sh
jot ls --notebook {{notebook}}
```

## Edit your own notebook

Opens in $EDITOR, or the system default if that is unset.

```sh @tags=edit
jot edit {{notebook}}
```

## Create a notebook

```sh @tags=edit
jot new {{name}}
```

## Open the data directory

Every notebook is a .md file in here. Edit them directly, keep them in git, whatever suits.

```sh @tags=edit
jot path
```

## Switch profile

A profile holds the constants of your own environment: service names, hosts, database names. Switching it changes every from: profile variable at once.

```sh @tags=profile
jot use {{profile}}
```

## Show the current profile's variables

```sh @tags=profile
jot profile
```

## Set a profile variable

```sh @tags=profile
jot profile set {{key}} {{value}}
```

## Install shell integration (PowerShell)

Add this one line to $PROFILE and Ctrl+J will type commands straight onto your prompt.

```ps1 @platform=windows @tags=setup
Add-Content $PROFILE "`njot init powershell | Out-String | Invoke-Expression"
```

## Install shell integration (bash)

```sh @platform=linux,macos @tags=setup
echo 'eval "$(jot init bash)"' >> ~/.bashrc
```

## Install shell integration (zsh)

```sh @platform=linux,macos @tags=setup
echo 'eval "$(jot init zsh)"' >> ~/.zshrc
```

## Self-check

Prints the data directory, notebook and entry counts, and the index time. Start here when something is off.

```sh
jot doctor
```

## Notebook format reference

A notebook is plain Markdown. Second-level headings are entry names, the paragraph below one is its description, and the code block is the command.

```txt @tags=reference
## Entry title

The description goes here: why you use it, what to watch out for.

​```sh @platform=linux @confirm @tags=deploy
sudo systemctl restart {{service}}
​```

Code block attributes:
  @platform=  windows / linux / macos, comma separated; other platforms are hidden
  @confirm    dangerous, asks once more before injecting
  @remote     meant for use after ssh; disables this entry's dynamic variables
  @tags=      extra search keywords
```

## Variable reference

A variable not declared in the frontmatter simply asks you each time.

```yaml @tags=reference
vars:
  service:
    desc: systemd unit
    from: profile          # take it from the active profile; ask if unset
    cmd: systemctl list-units --type=service --no-legend --plain | awk '{print $1}'
  branch:
    desc: branch
    from: shell            # run a command to produce the candidate list
    cmd: git branch --format=%(refname:short)
  env:
    desc: environment
    from: ask              # offer a fixed set of options
    options: ["Development", "Production"]

# Inline default:  {{port=8080}}
# Built-ins:       {{@cwd}} {{@clipboard}} {{@date}} {{@git.branch}} {{@env.NAME}}
# Literal braces:  \{{ is not a variable (Go templates like {{.Names}} are detected automatically, no escaping needed)
```
