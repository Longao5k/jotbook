//! The integration script printed by `jot init <shell>`.
//!
//! The user's profile only ever holds one `jot init ... | eval` line; the
//! script itself ships with the binary, so upgrades need no config edits.
//! Same approach as zoxide and starship.

pub fn script(shell: &str, key: Option<&str>) -> Option<String> {
    Some(match shell {
        "powershell" | "pwsh" => powershell(key.unwrap_or("Ctrl+j")),
        "bash" => bash(key.unwrap_or(r"\C-g")),
        "zsh" => zsh(key.unwrap_or("^G")),
        "fish" => fish(key.unwrap_or("\\cg")),
        _ => return None,
    })
}

pub const SHELLS: &[&str] = &["powershell", "bash", "zsh", "fish"];

fn powershell(key: &str) -> String {
    format!(
        r#"# jot shell integration (PowerShell)
function Invoke-JotWidget {{
    $line = $null
    $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)
    $env:JOT_WIDGET = '1'
    try {{
        $out = & jot pick --widget --line "$line"
        $code = $LASTEXITCODE
    }} finally {{
        Remove-Item Env:\JOT_WIDGET -ErrorAction SilentlyContinue
    }}
    if ($code -eq 0 -and $out) {{
        [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
        [Microsoft.PowerShell.PSConsoleReadLine]::Insert(($out -join "`n"))
    }}
}}
Set-PSReadLineKeyHandler -Chord '{key}' -ScriptBlock {{ Invoke-JotWidget }}
"#
    )
}

fn bash(key: &str) -> String {
    format!(
        r#"# jot shell integration (bash)
__jot_widget() {{
  local out
  out=$(JOT_WIDGET=1 jot pick --widget --line "$READLINE_LINE" </dev/tty) || return
  [ -z "$out" ] && return
  READLINE_LINE="$out"
  READLINE_POINT=${{#READLINE_LINE}}
}}
bind -x '"{key}": __jot_widget'
"#
    )
}

fn zsh(key: &str) -> String {
    format!(
        r#"# jot shell integration (zsh)
__jot_widget() {{
  local out
  out=$(JOT_WIDGET=1 jot pick --widget --line "$BUFFER" </dev/tty) || return
  [[ -z "$out" ]] && return
  BUFFER="$out"
  CURSOR=${{#BUFFER}}
  zle redisplay
}}
zle -N __jot_widget
bindkey '{key}' __jot_widget
"#
    )
}

fn fish(key: &str) -> String {
    format!(
        r#"# jot shell integration (fish)
function __jot_widget
    set -l out (JOT_WIDGET=1 jot pick --widget --line (commandline) 2>/dev/tty)
    or return
    test -z "$out"; and return
    commandline -r -- $out
end
bind {key} __jot_widget
"#
    )
}
