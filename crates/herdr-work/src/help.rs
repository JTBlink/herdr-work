//! Localize clap help while preserving parser validation and command syntax.
use clap::Command;

pub(crate) fn command(mut command: Command) -> Command {
    command.build();
    localize(command)
}

fn localize(command: Command) -> Command {
    let mut command = command;
    let usage = command.render_usage().to_string();
    let usage = usage
        .strip_prefix("Usage: ")
        .unwrap_or(&usage)
        .replace("[OPTIONS]", "[选项]")
        .replace("[COMMAND]", "[子命令]")
        .replace("<COMMAND>", "<子命令>");
    if command.get_name() == "help" {
        command = command.about("显示主命令或指定子命令的帮助");
    }
    command
        .help_template("{about-with-newline}\n用法：{usage}\n\n{all-args}{after-help}")
        .override_usage(usage)
        .subcommand_help_heading("子命令")
        .mut_args(|arg| {
            let positional = arg.is_positional();
            let arg = match arg.get_id().as_str() {
                "help" => arg.help("显示帮助").long_help("显示当前命令的详细帮助"),
                "version" => arg.help("显示版本"),
                _ => arg,
            };
            arg.help_heading(if positional { "参数" } else { "选项" })
        })
        .mut_subcommands(localize)
}
