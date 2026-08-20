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

# Windows PowerShell decodes a native command's stdout with this encoding, and
# it defaults to the OEM code page. jot emits UTF-8, so without this any
# non-ASCII in a command comes back mangled and gets typed onto your prompt
# that way.
if ([Console]::OutputEncoding.CodePage -ne 65001) {{
    [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
}}

function Invoke-JotWidget {{
    $line = $null
    $cursor = $null
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref]$line, [ref]$cursor)

    # Windows PowerShell drops empty-string arguments to native executables, so
    # passing --line "" makes clap see a flag with no value and bail. Only pass
    # it when there is something to pass; jot defaults it to empty anyway.
    $jotArgs = @('pick', '--widget')
    if ($line) {{ $jotArgs += @('--line', $line) }}

    $out = $null
    $code = $null
    $failure = $null
    $env:JOT_WIDGET = '1'
    try {{
        # Capture stdout only. Redirecting stderr with 2>&1 would put it in the
        # pipeline, and stderr is where the picker draws - jot would rightly
        # refuse to open a UI on a pipe. Anything jot writes to stderr lands on
        # the terminal, which is exactly where the user should see it.
        $out = & jot @jotArgs
        $code = $LASTEXITCODE
    }} catch {{
        $failure = $_
    }} finally {{
        Remove-Item Env:\JOT_WIDGET -ErrorAction SilentlyContinue
    }}

    # jot draws on the alternate screen, so PSReadLine's view is stale on return
    [Microsoft.PowerShell.PSConsoleReadLine]::InvokePrompt()

    if ($code -eq 0 -and $out) {{
        [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
        [Microsoft.PowerShell.PSConsoleReadLine]::Insert(($out -join "`n"))
        return
    }}
    # 130 is a deliberate cancel; anything else is a problem worth seeing.
    # A key handler that fails silently is impossible to diagnose.
    if ($code -ne 130) {{
        $why = if ($failure) {{ "$failure" }} elseif ($null -eq $code) {{ 'jot did not run' }} else {{ "exit $code" }}
        Write-Host ''
        Write-Host "jot: widget failed - $why" -ForegroundColor Yellow
        [Microsoft.PowerShell.PSConsoleReadLine]::InvokePrompt()
    }}
}}

Set-PSReadLineKeyHandler -Chord '{key}' -ScriptBlock {{ Invoke-JotWidget }}

# Ctrl+J is LF and some terminals never deliver it as a distinct key, so bind a
# second, unambiguous chord as well. Either one opens the picker.
Set-PSReadLineKeyHandler -Chord 'Alt+j' -ScriptBlock {{ Invoke-JotWidget }}
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
