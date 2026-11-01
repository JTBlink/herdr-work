use std::process::Command;

fn help(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_herdr-work"))
        .args(args)
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn main_help_explains_commands_and_global_options_in_chinese() {
    let text = help(&[]);
    for expected in [
        "用法",
        "子命令",
        "数据目录",
        "源码来源",
        "启动终端界面",
        "显示帮助",
        "显示版本",
    ] {
        assert!(text.contains(expected), "Missing {expected}: {text}");
    }
    assert!(!text.contains("Print help"));
}

#[test]
fn nested_help_explains_parameters_in_chinese() {
    for (args, expected) in [
        (vec!["diff"], "差异范围"),
        (vec!["review", "open"], "Git 工作目录"),
        (vec!["review", "add"], "起始行"),
        (vec!["review", "resolve"], "已送达"),
        (vec!["reconcile"], "核对依据"),
    ] {
        let text = help(&args);
        assert!(text.contains("用法"), "{text}");
        assert!(text.contains(expected), "Missing {expected}: {text}");
        assert!(!text.contains("Print help"), "{text}");
    }
}

#[test]
fn help_subcommand_and_short_flag_remain_available() {
    for args in [vec!["help", "review", "open"], vec!["diff", "-h"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_herdr-work"))
            .args(&args)
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("用法"), "{args:?}: {text}");
        assert!(!text.contains("Print help"), "{text}");
    }
}

#[test]
fn obsolete_import_commands_and_configuration_are_not_exposed() {
    let text = help(&[]);
    assert!(!text.contains("WORKDESK_HOME"));
    assert!(!text.contains("rollback-import"));
    for command in ["import", "rollback-import", "tasks", "task", "scheduler"] {
        let output = Command::new(env!("CARGO_BIN_EXE_herdr-work"))
            .arg(command)
            .arg("--help")
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "Removed command is still accepted: {command}"
        );
    }
}

#[test]
fn removed_backend_options_are_rejected_before_opening_a_profile() {
    for args in [
        vec!["--projects-backend", "unused", "doctor"],
        vec!["review", "open", "--task", "unused"],
        vec!["review", "open", "--execution", "unused"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_herdr-work"))
            .args(&args)
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "Removed options accepted: {args:?}"
        );
    }
}
