use std::ffi::OsStr;
use std::fmt::Write;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandKind {
    Help,
    Version,
    Doctor,
    Init,
    Graph,
    Fetch,
    Check,
    Build,
    Run,
    Test,
    Tokens,
    Ast,
    Fmt,
    Lsp,
}

impl CommandKind {
    pub(crate) fn from_invocation(value: &OsStr) -> Option<Self> {
        let value = value.to_str()?;
        COMMANDS
            .iter()
            .find(|schema| schema.name == value)
            .map(|schema| schema.kind)
    }

    pub(crate) fn schema(self) -> &'static CommandSchema {
        COMMANDS
            .iter()
            .find(|schema| schema.kind == self)
            .expect("every command kind has one schema")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandOption {
    Help,
    Root,
    File,
    Executable,
    Output,
    Locked,
    Offline,
    Format,
    Target,
    Test,
    Case,
    FormatCheck,
    Name,
    Library,
}

impl CommandOption {
    const fn bit(self) -> u16 {
        1 << (self as u16)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OptionSchema {
    option: CommandOption,
    long: &'static str,
    short: Option<&'static str>,
    value: Option<&'static str>,
    description: &'static str,
}

impl OptionSchema {
    pub(crate) const fn option(self) -> CommandOption {
        self.option
    }

    pub(crate) const fn canonical_name(self) -> &'static str {
        self.long
    }

    pub(crate) const fn takes_value(self) -> bool {
        self.value.is_some()
    }

    fn invocation(self) -> String {
        let names = match self.short {
            Some(short) => format!("{short}, {}", self.long),
            None => self.long.into(),
        };
        match self.value {
            Some(value) => format!("{names} <{value}>"),
            None => names,
        }
    }
}

pub(crate) fn option_schema(spelling: &str) -> Option<&'static OptionSchema> {
    OPTIONS
        .iter()
        .find(|schema| schema.long == spelling || schema.short == Some(spelling))
}

#[derive(Clone, Copy)]
struct PositionalSchema {
    value: &'static str,
    description: &'static str,
}

#[derive(Clone, Copy)]
struct TrailingArgumentsSchema {
    usage: &'static str,
    value: &'static str,
    description: &'static str,
}

#[derive(Clone, Copy)]
enum CommandForm {
    Subcommand,
    GlobalOption,
}

#[derive(Clone, Copy)]
pub(crate) struct CommandSchema {
    kind: CommandKind,
    name: &'static str,
    form: CommandForm,
    summary: &'static str,
    accepted: u16,
    positional: Option<PositionalSchema>,
    trailing_arguments: Option<TrailingArgumentsSchema>,
}

impl CommandSchema {
    pub(crate) const fn name(self) -> &'static str {
        self.name
    }

    pub(crate) const fn accepts(self, option: CommandOption) -> bool {
        self.accepted & option.bit() != 0
    }

    pub(crate) const fn accepts_positional(self) -> bool {
        self.positional.is_some()
    }

    fn usage(self) -> String {
        let mut usage = format!("nocter {}", self.name);
        if self.accepted != 0 {
            usage.push_str(" [OPTIONS]");
        }
        if let Some(positional) = self.positional {
            write!(usage, " [{}]", positional.value).expect("writing to String cannot fail");
        }
        if let Some(trailing) = self.trailing_arguments {
            write!(usage, " {}", trailing.usage).expect("writing to String cannot fail");
        }
        usage
    }
}

const HELP_OPTION: u16 = CommandOption::Help.bit();
const RESOLUTION_OPTIONS: u16 = CommandOption::Locked.bit() | CommandOption::Offline.bit();
const INPUT_OPTIONS: u16 =
    CommandOption::Root.bit() | CommandOption::File.bit() | CommandOption::Executable.bit();

const OPTIONS: [OptionSchema; 14] = [
    OptionSchema {
        option: CommandOption::Help,
        long: "--help",
        short: None,
        value: None,
        description: "Show help for this command.",
    },
    OptionSchema {
        option: CommandOption::Root,
        long: "--root",
        short: None,
        value: Some("DIR"),
        description: "Select the package rooted at DIR.",
    },
    OptionSchema {
        option: CommandOption::File,
        long: "--file",
        short: None,
        value: Some("SOURCE"),
        description: "Select one standalone .nct source file.",
    },
    OptionSchema {
        option: CommandOption::Executable,
        long: "--executable",
        short: None,
        value: Some("NAME"),
        description: "Select the named package executable.",
    },
    OptionSchema {
        option: CommandOption::Output,
        long: "--output",
        short: Some("-o"),
        value: Some("PATH"),
        description: "Write the selected executable to PATH.",
    },
    OptionSchema {
        option: CommandOption::Locked,
        long: "--locked",
        short: None,
        value: None,
        description: "Require every dependency to have an exact authored lock.",
    },
    OptionSchema {
        option: CommandOption::Offline,
        long: "--offline",
        short: None,
        value: None,
        description: "Forbid network package acquisition.",
    },
    OptionSchema {
        option: CommandOption::Format,
        long: "--format",
        short: None,
        value: Some("FORMAT"),
        description: "Use JSON output (json).",
    },
    OptionSchema {
        option: CommandOption::Target,
        long: "--target",
        short: None,
        value: Some("TARGET"),
        description: "Select the compilation target.",
    },
    OptionSchema {
        option: CommandOption::Test,
        long: "--test",
        short: None,
        value: Some("NAME"),
        description: "Select the named package test target.",
    },
    OptionSchema {
        option: CommandOption::Case,
        long: "--case",
        short: None,
        value: Some("NAME"),
        description: "Select one exact case in the named test target.",
    },
    OptionSchema {
        option: CommandOption::FormatCheck,
        long: "--check",
        short: None,
        value: None,
        description: "Report a formatting difference without rewriting the source.",
    },
    OptionSchema {
        option: CommandOption::Name,
        long: "--name",
        short: None,
        value: Some("NAME"),
        description: "Set the initialized package name.",
    },
    OptionSchema {
        option: CommandOption::Library,
        long: "--library",
        short: None,
        value: None,
        description: "Initialize a library package instead of an executable.",
    },
];

const SOURCE: PositionalSchema = PositionalSchema {
    value: "SOURCE",
    description: "A standalone .nct source file.",
};

const DIRECTORY: PositionalSchema = PositionalSchema {
    value: "DIR",
    description: "A new or existing directory for the package.",
};

const HELP_TOPIC: PositionalSchema = PositionalSchema {
    value: "COMMAND",
    description: "An implemented command or global option.",
};

const RUN_ARGUMENTS: TrailingArgumentsSchema = TrailingArgumentsSchema {
    usage: "[-- <ARG>...]",
    value: "ARG...",
    description: "Arguments forwarded unchanged to the launched program after --.",
};

const HELP: CommandSchema = CommandSchema {
    kind: CommandKind::Help,
    name: "help",
    form: CommandForm::Subcommand,
    summary: "Show overview or command-specific help.",
    accepted: HELP_OPTION,
    positional: Some(HELP_TOPIC),
    trailing_arguments: None,
};

const VERSION: CommandSchema = CommandSchema {
    kind: CommandKind::Version,
    name: "--version",
    form: CommandForm::GlobalOption,
    summary: "Report the validated compiler installation identity.",
    accepted: 0,
    positional: None,
    trailing_arguments: None,
};

const DOCTOR: CommandSchema = CommandSchema {
    kind: CommandKind::Doctor,
    name: "doctor",
    form: CommandForm::Subcommand,
    summary: "Validate and report the active Nocter home.",
    accepted: HELP_OPTION,
    positional: None,
    trailing_arguments: None,
};

const INIT: CommandSchema = CommandSchema {
    kind: CommandKind::Init,
    name: "init",
    form: CommandForm::Subcommand,
    summary: "Create a source-owned package without overwriting files.",
    accepted: HELP_OPTION | CommandOption::Name.bit() | CommandOption::Library.bit(),
    positional: Some(DIRECTORY),
    trailing_arguments: None,
};

const GRAPH: CommandSchema = CommandSchema {
    kind: CommandKind::Graph,
    name: "graph",
    form: CommandForm::Subcommand,
    summary: "Inspect one exact read-only package graph.",
    accepted: HELP_OPTION
        | CommandOption::Root.bit()
        | RESOLUTION_OPTIONS
        | CommandOption::Format.bit(),
    positional: None,
    trailing_arguments: None,
};

const FETCH: CommandSchema = CommandSchema {
    kind: CommandKind::Fetch,
    name: "fetch",
    form: CommandForm::Subcommand,
    summary: "Resolve dependencies and commit exact locks and package state.",
    accepted: HELP_OPTION | CommandOption::Root.bit() | RESOLUTION_OPTIONS,
    positional: None,
    trailing_arguments: None,
};

const CHECK: CommandSchema = CommandSchema {
    kind: CommandKind::Check,
    name: "check",
    form: CommandForm::Subcommand,
    summary: "Check one package or standalone source without emitting an executable.",
    accepted: HELP_OPTION
        | INPUT_OPTIONS
        | RESOLUTION_OPTIONS
        | CommandOption::Format.bit()
        | CommandOption::Target.bit(),
    positional: Some(SOURCE),
    trailing_arguments: None,
};

const BUILD: CommandSchema = CommandSchema {
    kind: CommandKind::Build,
    name: "build",
    form: CommandForm::Subcommand,
    summary: "Build one package or standalone source.",
    accepted: HELP_OPTION
        | INPUT_OPTIONS
        | CommandOption::Output.bit()
        | RESOLUTION_OPTIONS
        | CommandOption::Target.bit(),
    positional: Some(SOURCE),
    trailing_arguments: None,
};

const RUN: CommandSchema = CommandSchema {
    kind: CommandKind::Run,
    name: "run",
    form: CommandForm::Subcommand,
    summary: "Build and run one selected executable or standalone source.",
    accepted: HELP_OPTION | INPUT_OPTIONS | RESOLUTION_OPTIONS | CommandOption::Target.bit(),
    positional: Some(SOURCE),
    trailing_arguments: Some(RUN_ARGUMENTS),
};

const TEST: CommandSchema = CommandSchema {
    kind: CommandKind::Test,
    name: "test",
    form: CommandForm::Subcommand,
    summary: "Compile and run declared package test targets.",
    accepted: HELP_OPTION
        | CommandOption::Root.bit()
        | CommandOption::Test.bit()
        | CommandOption::Case.bit()
        | CommandOption::Target.bit()
        | RESOLUTION_OPTIONS
        | CommandOption::Format.bit(),
    positional: None,
    trailing_arguments: None,
};

const TOKENS: CommandSchema = CommandSchema {
    kind: CommandKind::Tokens,
    name: "tokens",
    form: CommandForm::Subcommand,
    summary: "Inspect one source file as a versioned lexical JSON envelope.",
    accepted: HELP_OPTION | CommandOption::Format.bit(),
    positional: Some(SOURCE),
    trailing_arguments: None,
};

const AST: CommandSchema = CommandSchema {
    kind: CommandKind::Ast,
    name: "ast",
    form: CommandForm::Subcommand,
    summary: "Inspect one source file as a versioned concrete-syntax JSON envelope.",
    accepted: HELP_OPTION | CommandOption::Format.bit(),
    positional: Some(SOURCE),
    trailing_arguments: None,
};

const FMT: CommandSchema = CommandSchema {
    kind: CommandKind::Fmt,
    name: "fmt",
    form: CommandForm::Subcommand,
    summary: "Format exactly one source file in place.",
    accepted: HELP_OPTION | CommandOption::FormatCheck.bit(),
    positional: Some(SOURCE),
    trailing_arguments: None,
};

const LSP: CommandSchema = CommandSchema {
    kind: CommandKind::Lsp,
    name: "lsp",
    form: CommandForm::Subcommand,
    summary: "Run the Language Server Protocol over standard input and output.",
    accepted: HELP_OPTION,
    positional: None,
    trailing_arguments: None,
};

const COMMANDS: [CommandSchema; 14] = [
    HELP, VERSION, DOCTOR, INIT, GRAPH, FETCH, CHECK, BUILD, RUN, TEST, TOKENS, AST, FMT, LSP,
];

/// One pure help selection produced by the public argument parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelpRequest {
    command: Option<CommandKind>,
}

impl HelpRequest {
    pub(crate) const fn overview() -> Self {
        Self { command: None }
    }

    pub(crate) const fn command(command: CommandKind) -> Self {
        Self {
            command: Some(command),
        }
    }

    #[must_use]
    pub fn render(self) -> String {
        match self.command {
            Some(command) => render_command_help(*command.schema()),
            None => render_overview(),
        }
    }
}

fn render_overview() -> String {
    let mut output = String::from(
        "Nocter compiler\n\nUsage:\n  nocter <COMMAND> [OPTIONS]\n  nocter --version\n  nocter --help\n\nCommands:\n",
    );
    for schema in COMMANDS
        .iter()
        .filter(|schema| matches!(schema.form, CommandForm::Subcommand))
    {
        writeln!(output, "  {:<10} {}", schema.name, schema.summary)
            .expect("writing to String cannot fail");
    }
    output.push_str("\nGlobal options:\n");
    writeln!(output, "  {:<10} Show this overview.", "--help")
        .expect("writing to String cannot fail");
    for schema in COMMANDS
        .iter()
        .filter(|schema| matches!(schema.form, CommandForm::GlobalOption))
    {
        writeln!(output, "  {:<10} {}", schema.name, schema.summary)
            .expect("writing to String cannot fail");
    }
    output
}

fn render_command_help(schema: CommandSchema) -> String {
    let mut output = format!("{}\n\nUsage:\n  {}\n", schema.summary, schema.usage());
    if let Some(positional) = schema.positional {
        write!(
            output,
            "\nArguments:\n  {:<22} {}\n",
            positional.value, positional.description,
        )
        .expect("writing to String cannot fail");
    }
    if let Some(trailing) = schema.trailing_arguments {
        if schema.positional.is_none() {
            output.push_str("\nArguments:\n");
        }
        writeln!(output, "  {:<22} {}", trailing.value, trailing.description)
            .expect("writing to String cannot fail");
    }
    let accepted_options = OPTIONS
        .iter()
        .copied()
        .filter(|option| schema.accepts(option.option));
    for (index, option) in accepted_options.enumerate() {
        if index == 0 {
            output.push_str("\nOptions:\n");
        }
        writeln!(
            output,
            "  {:<22} {}",
            option.invocation(),
            option.description,
        )
        .expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: [CommandKind; 14] = [
        CommandKind::Help,
        CommandKind::Version,
        CommandKind::Doctor,
        CommandKind::Init,
        CommandKind::Graph,
        CommandKind::Fetch,
        CommandKind::Check,
        CommandKind::Build,
        CommandKind::Run,
        CommandKind::Test,
        CommandKind::Tokens,
        CommandKind::Ast,
        CommandKind::Fmt,
        CommandKind::Lsp,
    ];

    #[test]
    fn command_and_option_schema_identities_are_complete_and_unique() {
        for kind in KINDS {
            assert_eq!(
                COMMANDS.iter().filter(|schema| schema.kind == kind).count(),
                1
            );
        }
        for (index, schema) in COMMANDS.iter().enumerate() {
            assert_eq!(
                CommandKind::from_invocation(OsStr::new(schema.name)),
                Some(schema.kind)
            );
            assert!(
                COMMANDS[index + 1..]
                    .iter()
                    .all(|another| schema.name != another.name)
            );
        }
        for (index, option) in OPTIONS.iter().enumerate() {
            assert!(OPTIONS[index + 1..].iter().all(|another| {
                option.long != another.long
                    && option.long != another.short.unwrap_or("")
                    && option.short.unwrap_or("") != another.long
                    && (option.short.is_none() || option.short != another.short)
            }));
        }
    }

    #[test]
    fn overview_contains_only_the_implemented_schema() {
        let rendered = HelpRequest::overview().render();

        for schema in COMMANDS {
            assert!(rendered.contains(schema.name));
        }
        assert!(rendered.contains("--help"));
        assert!(rendered.contains("init"));
        assert!(rendered.contains("graph"));
        assert!(rendered.contains("test"));
        assert!(!rendered.contains("nocter fmt"));
        assert!(rendered.contains("lsp"));
    }

    #[test]
    fn command_help_projects_exactly_the_accepted_option_set() {
        for command in COMMANDS {
            let rendered = render_command_help(command);
            assert!(rendered.contains(&command.usage()));
            for option in OPTIONS {
                assert_eq!(
                    rendered.contains(option.long),
                    command.accepts(option.option),
                    "{} / {}",
                    command.name,
                    option.long,
                );
            }
        }
    }

    #[test]
    fn only_run_help_advertises_the_opaque_program_argument_channel() {
        for command in COMMANDS {
            let rendered = render_command_help(command);
            assert_eq!(
                rendered.contains("[-- <ARG>...]"),
                command.kind == CommandKind::Run,
                "{}",
                command.name,
            );
        }
        let rendered = render_command_help(RUN);
        assert!(rendered.contains("forwarded unchanged"));
    }
}
