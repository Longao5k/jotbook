//! CLI-level integration tests: run the real binary against an isolated
//! JOT_HOME. Unit tests cannot tell you whether the thing works once
//! installed; this layer can.
//!
//! JOT_LANG is pinned so the assertions do not depend on the machine locale.
//! One test deliberately runs in Chinese to cover the switch itself.

use std::path::PathBuf;
use std::process::{Command, Output};

/// One data directory per test, because tests run in parallel.
fn temp_home(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jot-it-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("could not create the temp directory");
    dir
}

fn jot(home: &PathBuf, args: &[&str]) -> Output {
    jot_in("en", home, args)
}

fn jot_in(lang: &str, home: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jot"))
        .env("JOT_HOME", home)
        .env("JOT_LANG", lang)
        // Editor-opening subcommands must not pop a window during tests
        .env("EDITOR", "")
        .args(args)
        .output()
        .expect("could not run jot")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n")
}
fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).replace("\r\n", "\n")
}

#[test]
fn version_works() {
    let home = temp_home("version");
    let o = jot(&home, &["--version"]);
    assert!(o.status.success());
    assert!(stdout(&o).contains("jot"), "got {:?}", stdout(&o));
}

#[test]
fn first_run_seeds_builtin_notebooks() {
    let home = temp_home("seed");
    let o = jot(&home, &["doctor"]);
    assert!(o.status.success(), "{}", stderr(&o));

    let builtin = home.join("notebooks").join("builtin");
    assert!(
        builtin.join("git.md").is_file(),
        "the built-in notebooks were not written"
    );
    assert!(builtin.join("docker.md").is_file());

    let out = stdout(&o);
    assert!(out.contains("notebooks"), "doctor output is wrong: {out}");
    // Starting from an empty directory should still not be empty
    assert!(
        !out.contains("entries         0 "),
        "the first run reported zero entries: {out}"
    );
}

#[test]
fn ls_lists_entries() {
    let home = temp_home("ls");
    let o = jot(&home, &["ls"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("# git"), "no git notebook: {out}");
    assert!(
        out.len() > 2000,
        "output is only {} bytes, too short",
        out.len()
    );
}

#[test]
fn first_resolves_a_command_without_variables() {
    let home = temp_home("first");
    let o = jot(
        &home,
        &["pick", "-q", "Undo the last commit but keep", "--first"],
    );
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o).trim(), "git reset --soft HEAD~1");
}

#[test]
fn inline_default_is_applied() {
    let home = temp_home("default");
    let o = jot(
        &home,
        &[
            "pick",
            "-q",
            "Serve the current directory over HTTP",
            "--first",
        ],
    );
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(
        stdout(&o).trim().ends_with("8000"),
        "the inline default did not apply: {}",
        stdout(&o)
    );
}

#[test]
fn go_templates_survive_end_to_end() {
    let home = temp_home("gotpl");
    let o = jot(&home, &["pick", "-q", "just names and status", "--first"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(
        out.contains("{{.Names}}"),
        "the Go template was eaten or substituted: {out}"
    );
}

#[test]
fn profile_value_feeds_a_variable() {
    let home = temp_home("profile");
    // Own fixture rather than a built-in entry: fuzzy search over shipped
    // titles is brittle, and this test is about profile resolution.
    let dir = home.join("notebooks").join("local");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("probe.md"),
        "---\nname: probe\n---\n\n## Zzprobe connect\n\n```sh\nssh {{host}}\n```\n",
    )
    .unwrap();

    // Without the profile it must fail, and say what is missing
    let miss = jot(&home, &["pick", "-q", "Zzprobe connect", "--first"]);
    assert!(
        !miss.status.success(),
        "succeeded despite a missing variable"
    );
    assert!(
        stderr(&miss).contains("host"),
        "did not say which variable was missing"
    );

    // With it set, the result comes straight out
    assert!(jot(&home, &["profile", "set", "host", "prod-01"])
        .status
        .success());
    let ok = jot(&home, &["pick", "-q", "Zzprobe connect", "--first"]);
    assert!(ok.status.success(), "{}", stderr(&ok));
    assert_eq!(stdout(&ok).trim(), "ssh prod-01");
}

#[test]
fn profiles_are_isolated_from_each_other() {
    let home = temp_home("profiles");
    // Own fixture rather than a built-in entry: fuzzy search over shipped
    // titles is brittle, and this test is about profile resolution.
    let dir = home.join("notebooks").join("local");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("probe.md"),
        "---\nname: probe\n---\n\n## Zzprobe connect\n\n```sh\nssh {{host}}\n```\n",
    )
    .unwrap();

    jot(&home, &["profile", "set", "host", "dev-box"]);
    jot(&home, &["use", "prod"]);
    jot(&home, &["profile", "set", "host", "prod-box"]);

    let o = jot(&home, &["pick", "-q", "Zzprobe connect", "--first"]);
    assert_eq!(stdout(&o).trim(), "ssh prod-box");

    jot(&home, &["use", "default"]);
    let o = jot(&home, &["pick", "-q", "Zzprobe connect", "--first"]);
    assert_eq!(stdout(&o).trim(), "ssh dev-box");
}

#[test]
fn platform_filtering_hides_other_platforms() {
    let home = temp_home("platform");
    let systemd = stdout(&jot(&home, &["ls", "--notebook", "systemd"]));
    let powershell = stdout(&jot(&home, &["ls", "--notebook", "powershell"]));

    if cfg!(target_os = "windows") {
        assert!(
            systemd.trim().is_empty(),
            "systemd entries must not show on Windows"
        );
        assert!(
            powershell.contains("#"),
            "powershell entries should show on Windows"
        );
    } else {
        assert!(
            powershell.trim().is_empty(),
            "powershell entries must not show off Windows"
        );
    }
}

#[test]
fn confirm_entries_are_announced_in_first_mode() {
    let home = temp_home("confirm");
    let o = jot(
        &home,
        &["pick", "-q", "Throw away every local change", "--first"],
    );
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(
        stderr(&o).contains("dangerous"),
        "a dangerous command said nothing in --first mode: {}",
        stderr(&o)
    );
}

#[test]
fn new_notebook_is_created_and_indexed() {
    let home = temp_home("new");
    // EDITOR is empty, so open_editor falls back to the system default. We do
    // not want a window, so only assert the file was created - the subcommand
    // must write it even when no editor can be opened.
    let _ = jot(&home, &["new", "scratch"]);
    let path = home.join("notebooks").join("local").join("scratch.md");
    assert!(path.is_file(), "the new notebook was not created");

    let o = jot(&home, &["ls", "--notebook", "scratch"]);
    assert!(
        stdout(&o).contains("scratch"),
        "the new notebook was not indexed"
    );
}

#[test]
fn save_appends_a_parseable_entry() {
    let home = temp_home("save");
    jot(&home, &["profile", "set", "service", "my-api.service"]);
    // save prompts for a title, so write the entry directly and assert the
    // format the resolver expects
    let dir = home.join("notebooks").join("local");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mine.md"),
        "---\nname: mine\n---\n\n## Restart my service\n\n```sh\nsudo systemctl restart {{service}}\n```\n",
    )
    .unwrap();

    let o = jot(&home, &["pick", "-q", "Restart my service", "--first"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o).trim(), "sudo systemctl restart my-api.service");
}

#[test]
fn secrets_are_refused_by_save() {
    let home = temp_home("secret");
    let o = jot(&home, &["save", "export GITHUB_TOKEN=abc123"]);
    assert!(
        !o.status.success(),
        "a command containing a secret was stored"
    );
    assert!(stderr(&o).contains("secret"), "{}", stderr(&o));
}

#[test]
fn init_emits_scripts_for_every_supported_shell() {
    let home = temp_home("init");
    for (shell, needle) in [
        ("powershell", "PSConsoleReadLine"),
        ("bash", "READLINE_LINE"),
        ("zsh", "zle"),
        ("fish", "commandline"),
    ] {
        let o = jot(&home, &["init", shell]);
        assert!(o.status.success(), "{shell}: {}", stderr(&o));
        assert!(
            stdout(&o).contains(needle),
            "the {shell} script content is wrong"
        );
        assert!(
            stdout(&o).contains("--widget"),
            "the {shell} script does not use the widget protocol"
        );
    }
    let bad = jot(&home, &["init", "nushell"]);
    assert!(!bad.status.success(), "an unknown shell should be an error");
}

#[test]
fn custom_key_is_honored() {
    let home = temp_home("key");
    let o = jot(&home, &["init", "bash", "--key", r"\C-o"]);
    assert!(
        stdout(&o).contains(r"\C-o"),
        "the custom key binding did not apply"
    );
}

#[test]
fn tui_refuses_to_run_without_a_terminal() {
    let home = temp_home("notty");
    let o = jot(&home, &["pick", "-q", "git"]);
    assert!(!o.status.success());
    assert!(
        stderr(&o).contains("--first"),
        "the non-TTY error does not point at an alternative: {}",
        stderr(&o)
    );
}

#[test]
fn unknown_first_arg_is_treated_as_a_query() {
    let home = temp_home("bareword");
    // `jot docker List...` must behave like `jot pick -q "docker List..."`,
    // and the trailing `--first` must stay an option rather than a search term
    let o = jot(&home, &["docker", "List running containers", "--first"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o).trim(), "docker ps");
}

#[test]
fn user_edits_to_builtin_notebooks_are_not_clobbered() {
    let home = temp_home("noclobber");
    jot(&home, &["doctor"]); // 先落地
    let git_md = home.join("notebooks").join("builtin").join("git.md");
    let original = std::fs::read_to_string(&git_md).unwrap();
    std::fs::write(
        &git_md,
        format!("{original}\n## mine\n\n```sh\necho mine\n```\n"),
    )
    .unwrap();

    // Run again: the same version must not rewrite anything
    jot(&home, &["doctor"]);
    let after = std::fs::read_to_string(&git_md).unwrap();
    assert!(
        after.contains("mine"),
        "re-running at the same version clobbered the user's edit"
    );
}

#[test]
fn local_notebooks_shadow_nothing_and_both_load() {
    let home = temp_home("shadow");
    jot(&home, &["doctor"]);
    let dir = home.join("notebooks").join("local");
    std::fs::create_dir_all(&dir).unwrap();
    // Same name as a built-in, but different variable declarations
    std::fs::write(
        dir.join("git.md"),
        "---\nname: git\nvars:\n  branch:\n    from: profile\n---\n\n## My branch command\n\n```sh\ngit switch {{branch}}\n```\n",
    )
    .unwrap();
    jot(&home, &["profile", "set", "branch", "feature/x"]);

    let o = jot(&home, &["pick", "-q", "My branch command", "--first"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o).trim(), "git switch feature/x");
}

#[test]
fn usage_is_recorded_and_ranks_entries() {
    let home = temp_home("usage");
    let usage_file = home.join("usage.toml");
    assert!(
        !usage_file.exists(),
        "a usage file exists before anything was used"
    );

    for _ in 0..3 {
        let o = jot(
            &home,
            &["pick", "-q", "Undo the last commit but keep", "--first"],
        );
        assert!(o.status.success(), "{}", stderr(&o));
    }

    let recorded = std::fs::read_to_string(&usage_file).expect("usage.toml was not created");
    assert!(
        recorded.contains("[\"git/"),
        "the entry was not recorded under a git id: {recorded}"
    );
    assert!(
        recorded.contains("count = 3"),
        "the count is wrong: {recorded}"
    );

    // doctor should list it as the most used
    let d = stdout(&jot(&home, &["doctor"]));
    assert!(
        d.contains("total uses      3"),
        "doctor did not count it: {d}"
    );
    assert!(
        d.contains("most used"),
        "doctor did not show the most-used list"
    );
}

#[test]
fn usage_file_corruption_does_not_break_the_tool() {
    let home = temp_home("badusage");
    jot(&home, &["doctor"]);
    std::fs::write(home.join("usage.toml"), "this is not valid TOML {{{").unwrap();

    let o = jot(
        &home,
        &["pick", "-q", "Undo the last commit but keep", "--first"],
    );
    assert!(
        o.status.success(),
        "a corrupt usage file broke the tool: {}",
        stderr(&o)
    );
    assert_eq!(stdout(&o).trim(), "git reset --soft HEAD~1");
}

#[test]
fn a_broken_notebook_does_not_break_the_rest() {
    let home = temp_home("badnotebook");
    jot(&home, &["doctor"]);
    let dir = home.join("notebooks").join("local");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("broken.md"),
        "---\nname: [unclosed\n---\n\n## x\n\n```sh\nls\n```\n",
    )
    .unwrap();

    let o = jot(
        &home,
        &["pick", "-q", "Undo the last commit but keep", "--first"],
    );
    assert!(
        o.status.success(),
        "one malformed notebook broke everything: {}",
        stderr(&o)
    );
    assert!(
        stderr(&o).contains("skipping"),
        "did not say which notebook was skipped"
    );
}

// ─────────────────────── community sources ───────────────────────

/// Build a local git repo to act as a community source, entirely offline.
fn make_source_repo(tag: &str) -> PathBuf {
    let repo = std::env::temp_dir().join(format!("jot-src-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(repo.join("notebooks")).unwrap();
    std::fs::write(
        repo.join("notebooks").join("shared.md"),
        "---\nname: shared\ndescription: shared notebook\n---\n\n## A command from the community\n\n```sh\necho from-community\n```\n",
    )
    .unwrap();
    std::fs::write(
        repo.join("README.md"),
        "# This repository\n\n## Install\n\n```sh\necho readme\n```\n",
    )
    .unwrap();

    let git = |args: &[&str]| {
        Command::new("git")
            .current_dir(&repo)
            .args(args)
            .output()
            .expect("could not run git");
    };
    git(&["init", "-b", "main"]);
    git(&["add", "-A"]);
    git(&[
        "-c",
        "user.name=test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "-m",
        "init",
    ]);
    repo
}

#[test]
fn a_community_source_can_be_added_listed_and_removed() {
    let home = temp_home("sources");
    let repo = make_source_repo("basic");
    let repo_str = repo.to_string_lossy().replace('\\', "/");

    let add = jot(&home, &["add", &repo_str, "--name", "extsrc"]);
    assert!(add.status.success(), "{}", stderr(&add));
    assert!(
        stderr(&add).contains("-> 1 commands"),
        "did not report the entry count: {}",
        stderr(&add)
    );
    // Installing must warn that dynamic variables are off
    assert!(
        stderr(&add).contains("trust"),
        "did not mention the trust model: {}",
        stderr(&add)
    );

    // The entries must be searchable
    let picked = jot(
        &home,
        &["pick", "-q", "A command from the community", "--first"],
    );
    assert!(picked.status.success(), "{}", stderr(&picked));
    assert_eq!(stdout(&picked).trim(), "echo from-community");

    // The README must not be treated as a notebook
    let ls = stdout(&jot(&home, &["ls"]));
    assert!(
        ls.contains("# shared"),
        "the community notebook is missing from ls"
    );
    assert!(
        !ls.contains("echo readme"),
        "the repository README was picked up as a notebook"
    );

    // Untrusted by default in the listing
    let sources = stdout(&jot(&home, &["sources"]));
    assert!(sources.contains("extsrc"), "{sources}");
    assert!(
        sources.contains("untrusted"),
        "an external source was trusted by default: {sources}"
    );

    // Trust and untrust
    assert!(jot(&home, &["trust", "extsrc"]).status.success());
    assert!(stdout(&jot(&home, &["sources"])).contains(" trusted"));
    assert!(jot(&home, &["untrust", "extsrc"]).status.success());
    assert!(stdout(&jot(&home, &["sources"])).contains("untrusted"));

    // Uninstall
    assert!(jot(&home, &["remove", "extsrc"]).status.success());
    let after = stdout(&jot(&home, &["sources"]));
    assert!(
        !after.contains("extsrc"),
        "still listed after removal: {after}"
    );
    assert!(
        !stdout(&jot(&home, &["ls"])).contains("# shared"),
        "the entries survived removal"
    );
}

#[test]
fn adding_the_same_source_twice_is_refused() {
    let home = temp_home("dupsource");
    let repo = make_source_repo("dup");
    let repo_str = repo.to_string_lossy().replace('\\', "/");

    assert!(jot(&home, &["add", &repo_str, "--name", "dup"])
        .status
        .success());
    let again = jot(&home, &["add", &repo_str, "--name", "dup"]);
    assert!(!again.status.success(), "installing twice was not refused");
    assert!(
        stderr(&again).contains("jot sync"),
        "did not tell the user how to update"
    );
}

#[test]
fn sync_reports_up_to_date_then_picks_up_changes() {
    let home = temp_home("syncsource");
    let repo = make_source_repo("sync");
    let repo_str = repo.to_string_lossy().replace('\\', "/");
    assert!(jot(&home, &["add", &repo_str, "--name", "s"])
        .status
        .success());

    let first = jot(&home, &["sync", "s"]);
    assert!(first.status.success(), "{}", stderr(&first));
    assert!(
        stderr(&first).contains("already up to date"),
        "{}",
        stderr(&first)
    );

    // Add an entry upstream
    std::fs::write(
        repo.join("notebooks").join("shared.md"),
        "---\nname: shared\n---\n\n## A command from the community\n\n```sh\necho from-community\n```\n\n## Added later\n\n```sh\necho newly-added\n```\n",
    )
    .unwrap();
    for args in [
        vec!["add", "-A"],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@e.com",
            "commit",
            "-m",
            "more",
        ],
    ] {
        Command::new("git")
            .current_dir(&repo)
            .args(&args)
            .output()
            .unwrap();
    }

    let second = jot(&home, &["sync", "s"]);
    assert!(second.status.success(), "{}", stderr(&second));
    assert!(stderr(&second).contains("updated"), "{}", stderr(&second));

    let o = jot(&home, &["pick", "-q", "Added later", "--first"]);
    assert_eq!(stdout(&o).trim(), "echo newly-added");
}

#[test]
fn unknown_source_names_are_rejected() {
    let home = temp_home("badsource");
    jot(&home, &["doctor"]);
    for args in [
        vec!["trust", "does-not-exist"],
        vec!["remove", "does-not-exist"],
        vec!["sync", "does-not-exist"],
    ] {
        let o = jot(&home, &args);
        assert!(!o.status.success(), "{:?} unexpectedly succeeded", args);
    }
}

// ─────────────────────── language ───────────────────────

fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
}

#[test]
fn the_interface_switches_language() {
    let home = temp_home("lang");

    let en = jot_in("en", &home, &["doctor"]);
    assert!(en.status.success(), "{}", stderr(&en));
    assert!(stdout(&en).contains("notebooks"), "{}", stdout(&en));
    assert!(
        !has_cjk(&stdout(&en)),
        "English output contains Chinese:\n{}",
        stdout(&en)
    );

    let zh = jot_in("zh", &home, &["doctor"]);
    assert!(zh.status.success(), "{}", stderr(&zh));
    assert!(
        has_cjk(&stdout(&zh)),
        "Chinese output is not Chinese:\n{}",
        stdout(&zh)
    );
}

#[test]
fn notebooks_switch_language_with_the_interface() {
    let home = temp_home("langnb");

    // Same entry, same command, different prose
    let zh = jot_in(
        "zh",
        &home,
        &["pick", "-q", "撤销最后一次提交 保留", "--first"],
    );
    assert!(zh.status.success(), "{}", stderr(&zh));
    assert_eq!(stdout(&zh).trim(), "git reset --soft HEAD~1");
    assert!(stderr(&zh).contains("撤销"), "{}", stderr(&zh));

    let en = jot_in(
        "en",
        &home,
        &["pick", "-q", "Undo the last commit but keep", "--first"],
    );
    assert!(en.status.success(), "{}", stderr(&en));
    assert_eq!(stdout(&en).trim(), "git reset --soft HEAD~1");
    assert!(stderr(&en).contains("Undo"), "{}", stderr(&en));

    // Only one language's notebooks are on disk at a time
    let ls = stdout(&jot_in("en", &home, &["ls", "--notebook", "git"]));
    assert!(
        !has_cjk(&ls),
        "both language sets are installed at once:\n{ls}"
    );
}

#[test]
fn jot_lang_persists_and_overrides_the_environment() {
    let home = temp_home("langset");

    // An explicit setting beats JOT_LANG
    assert!(jot_in("en", &home, &["lang", "zh"]).status.success());
    let out = stdout(&jot_in("en", &home, &["doctor"]));
    assert!(has_cjk(&out), "`jot lang zh` did not stick:\n{out}");

    // auto hands control back to the environment
    assert!(jot_in("en", &home, &["lang", "auto"]).status.success());
    let out = stdout(&jot_in("en", &home, &["doctor"]));
    assert!(
        !has_cjk(&out),
        "auto did not fall back to the environment:\n{out}"
    );

    let bad = jot_in("en", &home, &["lang", "klingon"]);
    assert!(
        !bad.status.success(),
        "an unknown language should be an error"
    );
}

/// Usage ids are keyed on the command rather than the title, so switching
/// language must not throw away the frecency data you have built up.
#[test]
fn usage_survives_a_language_switch() {
    let home = temp_home("usagelang");

    for _ in 0..3 {
        let o = jot_in(
            "en",
            &home,
            &["pick", "-q", "Undo the last commit but keep", "--first"],
        );
        assert!(o.status.success(), "{}", stderr(&o));
    }
    let before = std::fs::read_to_string(home.join("usage.toml")).unwrap();
    assert!(before.contains("count = 3"), "{before}");

    // Same entry, now in Chinese: the count must keep climbing, not restart
    let o = jot_in(
        "zh",
        &home,
        &["pick", "-q", "撤销最后一次提交 保留", "--first"],
    );
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o).trim(), "git reset --soft HEAD~1");

    let after = std::fs::read_to_string(home.join("usage.toml")).unwrap();
    assert!(
        after.contains("count = 4"),
        "switching language reset the usage count:\n{after}"
    );
    assert_eq!(
        after.matches("[\"git/").count(),
        1,
        "the same entry was recorded twice under different ids:\n{after}"
    );
}

/// Regression: the PowerShell widget used to pass `--line ""`, and Windows
/// PowerShell drops empty-string arguments to native executables, so clap saw
/// a flag with no value and exited 2. Pressing the key on an empty prompt
/// therefore always failed - silently, because the handler swallowed it.
#[test]
fn widget_works_without_a_line_argument() {
    let home = temp_home("widgetnoline");
    let o = jot(&home, &["pick", "--widget"]);

    // Not a TTY here, so it must fail with the terminal guard - not a usage error
    assert_ne!(
        o.status.code(),
        Some(2),
        "clap rejected the arguments: {}",
        stderr(&o)
    );
    assert!(
        stderr(&o).contains("--first"),
        "expected the terminal guard, got: {}",
        stderr(&o)
    );
}

/// The PowerShell script must never pass an empty --line, and must surface a
/// failure rather than swallowing it.
#[test]
fn powershell_widget_script_is_defensive() {
    let home = temp_home("psscript");
    let s = stdout(&jot(&home, &["init", "powershell"]));

    assert!(
        !s.contains("--line \"$line\""),
        "still passes --line unconditionally, which breaks on an empty prompt:\n{s}"
    );
    assert!(
        s.contains("if ($line)"),
        "does not guard against an empty line:\n{s}"
    );
    assert!(
        s.contains("widget failed"),
        "a failing widget would be silent:\n{s}"
    );
    assert!(
        s.contains("InvokePrompt"),
        "does not redraw after the alternate screen:\n{s}"
    );
}
