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

# READLINE_LINE arrived in bash 4.0, and without it a key binding has no way to
# put anything on the prompt. macOS still ships 3.2 as /bin/bash, so say what
# happened rather than binding a key that silently does nothing.
if [ "${{BASH_VERSINFO[0]:-0}}" -lt 4 ]; then
  echo "jot: the key binding needs bash 4.0 or newer (this is bash $BASH_VERSION)" >&2
  echo "jot: everything else still works - run 'jot' to pick a command" >&2
else
  __jot_widget() {{
    local out
    out=$(JOT_WIDGET=1 jot pick --widget --line "$READLINE_LINE" </dev/tty) || return
    [ -z "$out" ] && return
    READLINE_LINE="$out"
    READLINE_POINT=${{#READLINE_LINE}}
  }}
  bind -x '"{key}": __jot_widget'
fi
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
    # `string collect` keeps the output as one value. Without it fish splits a
    # command substitution on newlines, and a two-line command comes back as two
    # list elements that get rejoined with a space - a different command from
    # the one that was picked. Roughly one entry in ten spans several lines.
    set -l out (JOT_WIDGET=1 jot pick --widget --line (commandline) </dev/tty | string collect)
    or return
    test -z "$out"; and return
    commandline -r -- $out
end
bind {key} __jot_widget
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_advertised_shell_produces_a_script() {
        for s in SHELLS {
            let script = script(s, None).unwrap_or_else(|| panic!("{s} advertised but unhandled"));
            assert!(script.contains("jot"), "{s} script never invokes jot");
        }
        assert!(script("tcsh", None).is_none());
    }

    /// The picker draws on the terminal device and reads keys from stdin, so
    /// every POSIX widget has to hand it the tty explicitly - the shell may
    /// well have stdin on a pipe.
    #[test]
    fn posix_widgets_give_the_picker_the_terminal() {
        for s in ["bash", "zsh", "fish"] {
            let script = script(s, None).unwrap();
            assert!(
                script.contains("</dev/tty"),
                "{s} never redirects stdin from the tty:\n{script}"
            );
        }
    }

    /// fish splits command substitution on newlines. `string collect` is what
    /// stops a two-line command from arriving as two arguments.
    #[test]
    fn fish_keeps_a_multi_line_command_in_one_piece() {
        let script = script("fish", None).unwrap();
        assert!(script.contains("string collect"), "{script}");
    }

    /// bash 3.2 - still /bin/bash on macOS - has no READLINE_LINE at all. A key
    /// that does nothing with no explanation is the worst possible outcome.
    #[test]
    fn bash_says_something_when_it_is_too_old_to_bind() {
        let script = script("bash", None).unwrap();
        assert!(
            script.contains("BASH_VERSINFO"),
            "no version guard:\n{script}"
        );
        assert!(
            script.contains("needs bash 4.0"),
            "no explanation:\n{script}"
        );
    }

    /// PowerShell hands back native stdout as an array of lines; joining it
    /// with a space would silently rewrite the command.
    #[test]
    fn powershell_rejoins_the_lines_it_split() {
        let script = script("powershell", None).unwrap();
        assert!(script.contains(r#"-join "`n""#), "{script}");
    }

    /// Redirecting stderr into the pipeline once broke the picker outright:
    /// jot saw a pipe and refused to draw. Nothing may put it back.
    ///
    /// Comments are skipped - every one of these scripts is allowed to explain
    /// why it does not do this, and one of them does.
    #[test]
    fn no_widget_captures_stderr() {
        for s in SHELLS {
            let script = script(s, None).unwrap();
            for (n, line) in script.lines().enumerate() {
                if line.trim_start().starts_with('#') {
                    continue;
                }
                assert!(
                    !line.contains("2>&1") && !line.contains("2>/dev/tty"),
                    "{s}:{} redirects stderr: {line:?}",
                    n + 1
                );
            }
        }
    }

    #[test]
    fn a_custom_key_reaches_the_binding() {
        assert!(script("bash", Some(r"\C-t")).unwrap().contains(r"\C-t"));
        assert!(script("zsh", Some("^T")).unwrap().contains("^T"));
        assert!(script("fish", Some("\\ct")).unwrap().contains("\\ct"));
        assert!(script("powershell", Some("Ctrl+t"))
            .unwrap()
            .contains("Ctrl+t"));
    }
}
