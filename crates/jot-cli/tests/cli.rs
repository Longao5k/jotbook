//! CLI 层的集成测试：真的把二进制跑起来，用隔离的 JOT_HOME。
//!
//! 单元测试覆盖不到「装好之后到底能不能用」，这一层补上。

use std::path::PathBuf;
use std::process::{Command, Output};

/// 每个测试一个独立的数据目录 —— 测试是并行跑的。
fn temp_home(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jot-it-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建不了临时目录");
    dir
}

fn jot(home: &PathBuf, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jot"))
        .env("JOT_HOME", home)
        // 编辑器相关的子命令不该在测试里弹窗
        .env("EDITOR", "")
        .args(args)
        .output()
        .expect("跑不起来 jot")
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
    assert!(stdout(&o).contains("jot"), "得到 {:?}", stdout(&o));
}

#[test]
fn first_run_seeds_builtin_notebooks() {
    let home = temp_home("seed");
    let o = jot(&home, &["doctor"]);
    assert!(o.status.success(), "{}", stderr(&o));

    let builtin = home.join("notebooks").join("builtin");
    assert!(builtin.join("git.md").is_file(), "内置笔记本没落地");
    assert!(builtin.join("docker.md").is_file());

    let out = stdout(&o);
    assert!(out.contains("笔记本"), "doctor 输出不对：{out}");
    // 空目录起步也不该是空的
    assert!(
        !out.contains("条目            0 条"),
        "首次运行条目数是 0：{out}"
    );
}

#[test]
fn ls_lists_entries() {
    let home = temp_home("ls");
    let o = jot(&home, &["ls"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("# git"), "没有 git 笔记本：{out}");
    assert!(out.len() > 2000, "输出太短，只有 {} 字节", out.len());
}

#[test]
fn first_resolves_a_command_without_variables() {
    let home = temp_home("first");
    let o = jot(&home, &["pick", "-q", "撤销最后一次提交 保留", "--first"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o).trim(), "git reset --soft HEAD~1");
}

#[test]
fn inline_default_is_applied() {
    let home = temp_home("default");
    let o = jot(&home, &["pick", "-q", "静态文件服务器", "--first"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(
        stdout(&o).trim().ends_with("8000"),
        "行内默认值没生效：{}",
        stdout(&o)
    );
}

#[test]
fn go_templates_survive_end_to_end() {
    let home = temp_home("gotpl");
    let o = jot(&home, &["pick", "-q", "容器名和状态", "--first"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(
        out.contains("{{.Names}}"),
        "Go 模板被吃掉了或被替换了：{out}"
    );
}

#[test]
fn profile_value_feeds_a_variable() {
    let home = temp_home("profile");
    // 没配 Profile 时应该失败，并说清楚缺什么
    let miss = jot(&home, &["pick", "-q", "ssh 登录", "--first"]);
    assert!(!miss.status.success(), "缺变量却成功了");
    assert!(stderr(&miss).contains("host"), "没指出缺哪个变量");

    // 配上之后应该直接出结果
    assert!(jot(&home, &["profile", "set", "host", "prod-01"])
        .status
        .success());
    let ok = jot(&home, &["pick", "-q", "ssh 登录", "--first"]);
    assert!(ok.status.success(), "{}", stderr(&ok));
    assert_eq!(stdout(&ok).trim(), "ssh prod-01");
}

#[test]
fn profiles_are_isolated_from_each_other() {
    let home = temp_home("profiles");
    jot(&home, &["profile", "set", "host", "dev-box"]);
    jot(&home, &["use", "prod"]);
    jot(&home, &["profile", "set", "host", "prod-box"]);

    let o = jot(&home, &["pick", "-q", "ssh 登录", "--first"]);
    assert_eq!(stdout(&o).trim(), "ssh prod-box");

    jot(&home, &["use", "default"]);
    let o = jot(&home, &["pick", "-q", "ssh 登录", "--first"]);
    assert_eq!(stdout(&o).trim(), "ssh dev-box");
}

#[test]
fn platform_filtering_hides_other_platforms() {
    let home = temp_home("platform");
    let systemd = stdout(&jot(&home, &["ls", "--notebook", "systemd"]));
    let powershell = stdout(&jot(&home, &["ls", "--notebook", "powershell"]));

    if cfg!(target_os = "windows") {
        assert!(systemd.trim().is_empty(), "Windows 上不该看到 systemd 条目");
        assert!(
            powershell.contains("#"),
            "Windows 上应该看到 powershell 条目"
        );
    } else {
        assert!(
            powershell.trim().is_empty(),
            "非 Windows 上不该看到 powershell 条目"
        );
    }
}

#[test]
fn confirm_entries_are_announced_in_first_mode() {
    let home = temp_home("confirm");
    let o = jot(&home, &["pick", "-q", "彻底丢弃所有本地改动", "--first"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(
        stderr(&o).contains("危险"),
        "危险命令没有在 --first 模式下出声：{}",
        stderr(&o)
    );
}

#[test]
fn new_notebook_is_created_and_indexed() {
    let home = temp_home("new");
    // EDITOR 设为空 → open_editor 会走系统默认，测试里不希望弹窗，
    // 所以只验证文件被创建（子命令本身即使打不开编辑器也应建好文件）
    let _ = jot(&home, &["new", "scratch"]);
    let path = home.join("notebooks").join("local").join("scratch.md");
    assert!(path.is_file(), "新笔记本没建出来");

    let o = jot(&home, &["ls", "--notebook", "scratch"]);
    assert!(stdout(&o).contains("scratch"), "新笔记本没被索引到");
}

#[test]
fn save_appends_a_parseable_entry() {
    let home = temp_home("save");
    jot(&home, &["profile", "set", "service", "my-api.service"]);
    // save 需要交互问标题，这里直接验证底层写入格式：先建笔记本再手写一条
    let dir = home.join("notebooks").join("local");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mine.md"),
        "---\nname: mine\n---\n\n## 重启我的服务\n\n```sh\nsudo systemctl restart {{service}}\n```\n",
    )
    .unwrap();

    let o = jot(&home, &["pick", "-q", "重启我的服务", "--first"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o).trim(), "sudo systemctl restart my-api.service");
}

#[test]
fn secrets_are_refused_by_save() {
    let home = temp_home("secret");
    let o = jot(&home, &["save", "export GITHUB_TOKEN=abc123"]);
    assert!(!o.status.success(), "含密钥的命令被存下来了");
    assert!(stderr(&o).contains("密钥"), "{}", stderr(&o));
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
        assert!(stdout(&o).contains(needle), "{shell} 脚本内容不对");
        assert!(
            stdout(&o).contains("--widget"),
            "{shell} 脚本没有用 widget 协议"
        );
    }
    let bad = jot(&home, &["init", "nushell"]);
    assert!(!bad.status.success(), "未知 shell 应该报错");
}

#[test]
fn custom_key_is_honored() {
    let home = temp_home("key");
    let o = jot(&home, &["init", "bash", "--key", r"\C-o"]);
    assert!(stdout(&o).contains(r"\C-o"), "自定义快捷键没生效");
}

#[test]
fn tui_refuses_to_run_without_a_terminal() {
    let home = temp_home("notty");
    let o = jot(&home, &["pick", "-q", "git"]);
    assert!(!o.status.success());
    assert!(
        stderr(&o).contains("--first"),
        "非 TTY 的报错没有指出可用的替代方案：{}",
        stderr(&o)
    );
}

#[test]
fn unknown_first_arg_is_treated_as_a_query() {
    let home = temp_home("bareword");
    // `jot docker 列出…` 应该等价于 `jot pick -q "docker 列出…"`，
    // 且末尾的 `--first` 必须当成选项，不能被吞进搜索词
    let o = jot(&home, &["docker", "列出运行中的容器", "--first"]);
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
        format!("{original}\n## 我加的\n\n```sh\necho mine\n```\n"),
    )
    .unwrap();

    // 再跑一次，同版本不该重写
    jot(&home, &["doctor"]);
    let after = std::fs::read_to_string(&git_md).unwrap();
    assert!(after.contains("我加的"), "同版本重跑把用户的修改冲掉了");
}

#[test]
fn local_notebooks_shadow_nothing_and_both_load() {
    let home = temp_home("shadow");
    jot(&home, &["doctor"]);
    let dir = home.join("notebooks").join("local");
    std::fs::create_dir_all(&dir).unwrap();
    // 和内置同名，但变量声明不同
    std::fs::write(
        dir.join("git.md"),
        "---\nname: git\nvars:\n  branch:\n    from: profile\n---\n\n## 我的分支命令\n\n```sh\ngit switch {{branch}}\n```\n",
    )
    .unwrap();
    jot(&home, &["profile", "set", "branch", "feature/x"]);

    let o = jot(&home, &["pick", "-q", "我的分支命令", "--first"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert_eq!(stdout(&o).trim(), "git switch feature/x");
}

#[test]
fn usage_is_recorded_and_ranks_entries() {
    let home = temp_home("usage");
    let usage_file = home.join("usage.toml");
    assert!(!usage_file.exists(), "还没用过就有统计文件");

    for _ in 0..3 {
        let o = jot(&home, &["pick", "-q", "撤销最后一次提交 保留", "--first"]);
        assert!(o.status.success(), "{}", stderr(&o));
    }

    let recorded = std::fs::read_to_string(&usage_file).expect("没生成 usage.toml");
    assert!(
        recorded.contains("git/撤销最后一次提交，但保留改动"),
        "条目没被记下来：{recorded}"
    );
    assert!(recorded.contains("count = 3"), "次数不对：{recorded}");

    // doctor 应该把它列为最常用
    let d = stdout(&jot(&home, &["doctor"]));
    assert!(d.contains("累计使用        3 次"), "doctor 没统计到：{d}");
    assert!(d.contains("最常用"), "doctor 没显示最常用列表");
}

#[test]
fn usage_file_corruption_does_not_break_the_tool() {
    let home = temp_home("badusage");
    jot(&home, &["doctor"]);
    std::fs::write(home.join("usage.toml"), "这不是合法的 TOML {{{").unwrap();

    let o = jot(&home, &["pick", "-q", "撤销最后一次提交 保留", "--first"]);
    assert!(
        o.status.success(),
        "统计文件损坏导致工具挂了：{}",
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

    let o = jot(&home, &["pick", "-q", "撤销最后一次提交 保留", "--first"]);
    assert!(
        o.status.success(),
        "一本笔记本写坏就整个用不了：{}",
        stderr(&o)
    );
    assert!(stderr(&o).contains("跳过"), "没有提示哪本笔记本被跳过了");
}

// ─────────────────────── 社区源 ───────────────────────

/// 造一个本地 git 仓库当社区源，全程离线。
fn make_source_repo(tag: &str) -> PathBuf {
    let repo = std::env::temp_dir().join(format!("jot-src-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&repo);
    std::fs::create_dir_all(repo.join("notebooks")).unwrap();
    std::fs::write(
        repo.join("notebooks").join("shared.md"),
        "---\nname: shared\ndescription: 共享笔记本\n---\n\n## 社区提供的命令\n\n```sh\necho from-community\n```\n",
    )
    .unwrap();
    std::fs::write(
        repo.join("README.md"),
        "# 这个仓库\n\n## 安装\n\n```sh\necho readme\n```\n",
    )
    .unwrap();

    let git = |args: &[&str]| {
        Command::new("git")
            .current_dir(&repo)
            .args(args)
            .output()
            .expect("git 跑不起来");
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

    let add = jot(&home, &["add", &repo_str, "--name", "community"]);
    assert!(add.status.success(), "{}", stderr(&add));
    assert!(
        stderr(&add).contains("1 条"),
        "没报告条目数：{}",
        stderr(&add)
    );
    // 装完必须提醒动态变量是禁用的
    assert!(
        stderr(&add).contains("trust"),
        "没提示信任模型：{}",
        stderr(&add)
    );

    // 条目要能被搜到
    let picked = jot(&home, &["pick", "-q", "社区提供的命令", "--first"]);
    assert!(picked.status.success(), "{}", stderr(&picked));
    assert_eq!(stdout(&picked).trim(), "echo from-community");

    // README 不该被当成笔记本
    let ls = stdout(&jot(&home, &["ls"]));
    assert!(ls.contains("# shared"), "社区笔记本没出现在 ls 里");
    assert!(!ls.contains("echo readme"), "仓库 README 被当成笔记本收了");

    // 列表里默认是未授信
    let sources = stdout(&jot(&home, &["sources"]));
    assert!(sources.contains("community"), "{sources}");
    assert!(
        sources.contains("未授信"),
        "外部源默认就被信任了：{sources}"
    );

    // 授信 / 撤销
    assert!(jot(&home, &["trust", "community"]).status.success());
    assert!(stdout(&jot(&home, &["sources"])).contains("已授信"));
    assert!(jot(&home, &["untrust", "community"]).status.success());
    assert!(stdout(&jot(&home, &["sources"])).contains("未授信"));

    // 卸载
    assert!(jot(&home, &["remove", "community"]).status.success());
    let after = stdout(&jot(&home, &["sources"]));
    assert!(!after.contains("community"), "卸载后还在：{after}");
    assert!(
        !stdout(&jot(&home, &["ls"])).contains("# shared"),
        "卸载后条目还在"
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
    assert!(!again.status.success(), "重复安装没被拒绝");
    assert!(stderr(&again).contains("jot sync"), "没告诉用户该怎么更新");
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
    assert!(stderr(&first).contains("已是最新"), "{}", stderr(&first));

    // 上游加一条
    std::fs::write(
        repo.join("notebooks").join("shared.md"),
        "---\nname: shared\n---\n\n## 社区提供的命令\n\n```sh\necho from-community\n```\n\n## 后来加的\n\n```sh\necho newly-added\n```\n",
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
    assert!(stderr(&second).contains("已更新"), "{}", stderr(&second));

    let o = jot(&home, &["pick", "-q", "后来加的", "--first"]);
    assert_eq!(stdout(&o).trim(), "echo newly-added");
}

#[test]
fn unknown_source_names_are_rejected() {
    let home = temp_home("badsource");
    jot(&home, &["doctor"]);
    for args in [
        vec!["trust", "不存在"],
        vec!["remove", "不存在"],
        vec!["sync", "不存在"],
    ] {
        let o = jot(&home, &args);
        assert!(!o.status.success(), "{:?} 居然成功了", args);
    }
}
