use crate::{
    Error, Harness,
    aggregate::{Handoff, Summary},
    render, resolve,
    transcript::{self, Selection},
};
use clap::{Arg, ArgAction, ArgMatches, Command};
use cli_command_kit::{HelpOptions, Mode};
use std::{
    ffi::OsString,
    fs::File,
    io::{BufReader, Write},
    path::Path,
};

pub const SKILL_POINTER: &str = "Install the agent-print-session skill using the Skills CLI:\n\n  npx skills add alexgorbatchev/agent-print-session-cli --skill agent-print-session\n\nSkill:\n\n  https://github.com/alexgorbatchev/agent-print-session-cli/tree/main/skills/agent-print-session\n";
pub fn help_options() -> HelpOptions {
    HelpOptions {
        skill: Some(SKILL_POINTER.into()),
        ..Default::default()
    }
}
fn common(command: Command, required: bool) -> Command {
    let target = Arg::new("target").value_name("session-id-or-path");
    let target = if required {
        target.required_unless_present("path")
    } else {
        target
    };
    command
        .arg(target)
        .arg(
            Arg::new("path")
                .long("path")
                .value_name("path")
                .value_parser(clap::builder::NonEmptyStringValueParser::new())
                .help("Direct transcript file path; overrides target"),
        )
        .arg(
            Arg::new("json")
                .long("json")
                .action(ArgAction::Set)
                .num_args(0..=1)
                .require_equals(true)
                .default_missing_value("true")
                .default_value("false")
                .value_parser(clap::value_parser!(bool))
                .help("Output formatted JSON"),
        )
}
fn print_flags(mut command: Command) -> Command {
    for (name, help) in [
        ("summary", "Summarize the full session"),
        (
            "handoff",
            "Build continuation context from the full session",
        ),
        ("last-turn", "Select the last conversational turn"),
        ("errors", "Select errors and failed tools"),
        ("files", "Select file modifications"),
        ("tools", "Select tool calls and results"),
        ("prompts", "Select user and assistant messages"),
        ("all", "Output all selected events (also the default)"),
    ] {
        command = command.arg(
            Arg::new(name)
                .long(name)
                .help(help)
                .action(ArgAction::Set)
                .num_args(0..=1)
                .require_equals(true)
                .default_missing_value("true")
                .default_value("false")
                .value_parser(clap::value_parser!(bool)),
        );
    }
    for (name, help) in [
        ("tail", "Select last N events; nonpositive means unlimited"),
        (
            "limit",
            "Select first N events; nonpositive means unlimited",
        ),
    ] {
        command = command.arg(
            Arg::new(name)
                .long(name)
                .help(help)
                .value_name("N")
                .default_value("0")
                .allow_hyphen_values(true)
                .value_parser(clap::value_parser!(i64)),
        );
    }
    command
}
pub fn command(mode: Mode) -> Command {
    let mut root = Command::new("agent-print-session")
        .version(env!("CARGO_PKG_VERSION"))
        .disable_version_flag(true)
        .arg(
            Arg::new("version")
                .long("version")
                .short('v')
                .action(ArgAction::SetTrue)
                .exclusive(true)
                .help("Print raw version"),
        )
        .about("Print and inspect local AI coding agent session logs");
    for (name, description) in [
        ("claude", "Inspect Claude Code transcripts"),
        ("codex", "Inspect Codex transcripts"),
        ("pi", "Inspect Pi transcripts"),
    ] {
        let mut harness = print_flags(common(Command::new(name).about(description), false))
            .subcommand_precedence_over_arg(true)
            .args_conflicts_with_subcommands(true);
        // Shorthand targets are optional, whereas explicit verbs need a target or --path.
        for (verb, about) in [
            ("print", "Print transcript events"),
            ("summary", "Aggregate session metrics"),
            ("handoff", "Build continuation context"),
        ] {
            let mut child = common(Command::new(verb).about(about), true);
            if verb == "print" {
                child = print_flags(child);
            }
            if verb == "handoff" {
                child = child.visible_aliases(["continue", "resume"]);
            }
            harness = harness.subcommand(child);
        }
        root = root.subcommand(harness);
    }
    root = root.subcommand(
        Command::new("completion")
            .about("Generate shell completion scripts")
            .subcommands(
                ["bash", "zsh", "fish", "powershell"]
                    .map(|name| Command::new(name).about(format!("Generate {name} completion"))),
            ),
    );
    cli_command_kit::setup(root, mode, &help_options()).expect("the static command tree is valid")
}
pub fn run(
    args: impl IntoIterator<Item = impl Into<OsString> + Clone>,
    mode: Mode,
    mut out: &mut dyn Write,
) -> Result<(), Error> {
    let mut command = command(mode);
    let matches = match command.clone().try_get_matches_from(args) {
        Ok(matches) => matches,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            write!(out, "{error}")?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    if matches.get_flag("version") {
        writeln!(out, "{}", env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    if cli_command_kit::write_skill(&matches, &help_options(), &mut out)? {
        return Ok(());
    }
    let Some((name, harness_matches)) = matches.subcommand() else {
        write!(out, "{}", command.render_help())?;
        return Ok(());
    };
    if name == "completion" {
        let Some((shell, _)) = harness_matches.subcommand() else {
            write!(
                out,
                "{}",
                command
                    .find_subcommand_mut("completion")
                    .expect("completion exists")
                    .render_help()
            )?;
            return Ok(());
        };
        let shell = match shell {
            "bash" => clap_complete::Shell::Bash,
            "zsh" => clap_complete::Shell::Zsh,
            "fish" => clap_complete::Shell::Fish,
            _ => clap_complete::Shell::PowerShell,
        };
        // clap_complete's writer API panics on write failures; buffer the generated
        // script and propagate the actual stdout failure from write_all instead.
        let mut bytes = Vec::new();
        clap_complete::generate(shell, &mut command, "agent-print-session", &mut bytes);
        out.write_all(&bytes)?;
        return Ok(());
    }
    let harness = match name {
        "claude" => Harness::Claude,
        "codex" => Harness::Codex,
        _ => Harness::Pi,
    };
    let (verb, matches) = harness_matches
        .subcommand()
        .unwrap_or(("print", harness_matches));
    if matches.get_one::<String>("target").is_none() && matches.get_one::<String>("path").is_none()
    {
        write!(
            out,
            "{}",
            command
                .find_subcommand_mut(name)
                .expect("harness exists")
                .render_help()
        )?;
        return Ok(());
    }
    execute(harness, verb, matches, mode, out)
}
fn boolean(matches: &ArgMatches, name: &str) -> bool {
    matches
        .try_get_one::<bool>(name)
        .ok()
        .flatten()
        .copied()
        .unwrap_or(false)
}
fn execute(
    harness: Harness,
    verb: &str,
    matches: &ArgMatches,
    mode: Mode,
    out: &mut dyn Write,
) -> Result<(), Error> {
    let explicit = matches.get_one::<String>("path");
    let target = explicit
        .or(matches.get_one::<String>("target"))
        .expect("target validated");
    let path = if explicit.is_some() {
        Path::new(target).to_path_buf()
    } else {
        resolve::resolve_in(target, &resolve::directories(harness))?
    };
    let file = File::open(&path).map_err(|error| {
        std::io::Error::new(
            error.kind(),
            format!(
                "opening transcript {}: {error}",
                cli_command_kit::display_path(&path)
            ),
        )
    })?;
    let events = transcript::load(BufReader::new(file), harness)?;
    let file = path.to_string_lossy();
    let width = cli_command_kit::terminal_width();
    let width = if width == 0 { 80 } else { width };
    let json = boolean(matches, "json");
    if verb == "handoff" || boolean(matches, "handoff") {
        let context = Handoff::compute(&file, &events, 5);
        if json {
            render::json(out, &context)
        } else {
            render::handoff(out, &context, mode, width)
        }
    } else if verb == "summary" || boolean(matches, "summary") {
        let summary = Summary::compute(&file, &events);
        if json {
            render::json(out, &summary)
        } else {
            render::summary(out, &summary, mode, width)
        }
    } else {
        let selected = transcript::select(
            &events,
            &Selection {
                last_turn: boolean(matches, "last-turn"),
                errors: boolean(matches, "errors"),
                files: boolean(matches, "files"),
                tools: boolean(matches, "tools"),
                prompts: boolean(matches, "prompts"),
                tail: *matches.get_one::<i64>("tail").expect("tail default"),
                limit: *matches.get_one::<i64>("limit").expect("limit default"),
            },
        );
        if json {
            render::json(out, &selected)
        } else {
            render::transcript(out, &file, &selected, mode, width)
        }
    }
}

pub fn report(error: &Error, mode: Mode, out: &mut dyn Write) -> std::io::Result<()> {
    let message = match error {
        Error::NotFound(value) => format!(
            "session not found: {}",
            cli_command_kit::display_path(Path::new(value))
        ),
        _ => error.to_string(),
    };
    write!(
        out,
        "{}{}",
        if mode == Mode::Agent {
            "ERR: "
        } else {
            "[ERROR] "
        },
        message.trim_end()
    )?;
    writeln!(out)
}
