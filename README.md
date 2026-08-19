# Jotbook

**English** · [简体中文](README.zh-CN.md)

A notebook for the commands **you** actually use. Hit one key, the command lands on your prompt — you press Enter.

```
┌ jot  4/366 ──────────────────────────────────────┐
│ › docker logs                                    │
└──────────────────────────────────────────────────┘
❯ Follow container logs                     docker
    docker logs -f --tail 200 ⟨container⟩
  Recent logs with timestamps               docker
    docker logs --timestamps --since 30m ⟨container⟩
───────────────────────────────────────────────────
 ↑↓ move   ⏎ use   ^E open file   esc cancel
```

Pick it, fill in the blanks, and the finished command is sitting on your command line:

```
$ docker logs -f --tail 200 kestrel-api▏
```

## Why

Not *"I don't know how to write this command"* — AI already solved that.

It's *"I know it, I used it last week, and I don't want to scroll through my chat history again."*
That's a **retrieval** problem, not a generation problem. And `sudo systemctl restart kestrel-orders-api.service`
— the exact command for *your* box — was never something a nondeterministic model should be regenerating.

## Features

- **Not empty on day one** — 13 notebooks, 460 commands built in: git, docker, flutter, dotnet, systemd, ssh, powershell, linux, npm, python, postgres, kubectl
- **Variables** — `{{service}}` can come from your profile, from a live command's output, or just ask you
- **Injects, never executes** — jot puts the command on your prompt and stops there. You press Enter
- **Plain text** — everything is Markdown. Sync with git, read it on GitHub, edit it in vim
- **No account, no server, no telemetry**
- **First-class on Windows** — bundled fuzzy matcher and TUI, no fzf dependency

## Install

```bash
cargo install --path crates/jot-cli
```

Then wire up your shell. One line — the script itself ships with the binary, so upgrades never require touching your config again.

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

Restart your shell, then press **<kbd>Ctrl</kbd>+<kbd>J</kbd>** (PowerShell) or **<kbd>Ctrl</kbd>+<kbd>G</kbd>** (bash / zsh / fish).

> Different keys on purpose: `Ctrl`+`J` is LF in a terminal, which readline treats as Enter — binding it there would break newlines. PSReadLine gets a distinct key event, so it's safe. Override with `jot init bash --key '\C-o'`.

## Getting started

```bash
jot import history --top 40                              # pull your most-used commands out of shell history
jot profile set service kestrel-orders-api.service   # your environment's constants
jot                                                      # go
```

## Commands

| Command | What it does |
|---|---|
| `jot` | Open the picker |
| `jot docker logs` | Open it with a search term |
| `jot save` | Save the command you just ran |
| `jot save "<command>"` | Save a specific command |
| `jot import history` | Bulk import from shell history, ranked by frequency |
| `jot ls` | List every entry |
| `jot edit [notebook]` | Open in `$EDITOR` |
| `jot new <name>` | Create a notebook |
| `jot use <profile>` | Switch profile |
| `jot profile set <k> <v>` | Set a profile variable |
| `jot doctor` | Self-check |
| `jot path` | Print the data directory |
| `jot pick -q "<term>" --first` | No UI, best match straight to stdout (for scripts) |

## Writing notebooks

A notebook is plain Markdown. `##` headings are entry names, the paragraph under one is the description, the code block is the command.

````markdown
---
name: my-servers
description: My boxes
vars:
  service:
    desc: systemd unit
    from: profile          # use the profile value; fall back to asking if unset
    cmd: systemctl list-units --type=service --no-legend --plain | awk '{print $1}'
---

## Restart the API

Required after changing appsettings. Not required for nginx config.

```sh @platform=linux @confirm @tags=deploy
sudo systemctl restart {{service}}
```
````

### Code block attributes

| Attribute | Effect |
|---|---|
| `@platform=` | `windows` / `linux` / `macos`, comma separated. Other platforms' entries are hidden |
| `@confirm` | Dangerous. Extra confirmation before injecting, marked ⚠ in the list |
| `@remote` | Meant for use after `ssh`. Disables this entry's dynamic variables, since they'd evaluate **locally** |
| `@tags=` | Extra search keywords |

### Variables

| Source | Behavior |
|---|---|
| `ask` (default) | Prompts you. Add `options` for a fixed list |
| `profile` | Taken from the active profile without interrupting you; falls back if unset |
| `shell` | Runs `cmd`, turns the output into a searchable candidate list |

Built-ins: `{{@cwd}}` `{{@date}}` `{{@host}}` `{{@git.branch}}` `{{@git.root}}` `{{@env.NAME}}`
Inline defaults: `{{port=8080}}`

**Go templates work as-is.** `{{ }}` collides with Docker and kubectl, so jot only treats *plain identifiers* as variables:

```sh
docker ps --format "{{.Names}}\t{{.Status}}"       # literal — no escaping needed
kubectl get x -o go-template='{{range .items}}…'   # literal
sudo systemctl restart {{service}}                  # this one is a variable
```

For a literal `{{name}}`, write `\{{name}}`.

## Where things live

```
~/.jot/
├── notebooks/
│   ├── builtin/     shipped with the binary; rewritten on upgrade
│   └── local/       yours; never touched
├── config.toml      active profile
└── profiles.toml    your environment's constants — not for secrets
```

`notebooks/` is a plain directory — `git init` it and you have sync, history, and sharing. `profiles.toml` deliberately sits outside it.

## Contributing

Notebooks are the most valuable thing you can send. A PR adding commands to `notebooks/*.md` needs no Rust at all — just Markdown that follows the format above. Commands that people genuinely can't remember are worth more than exhaustive coverage of `--help`.

For the client itself, see [docs/design.html](docs/design.html) for architecture, decision records, and non-goals before opening a large PR.

### Building

```bash
cargo test
cargo build --release
```

On Windows, use the MSVC toolchain — the GNU host needs a full MinGW install because `dlltool` shells out to `as`:

```bash
rustup default stable-x86_64-pc-windows-msvc
```

## License

MIT
