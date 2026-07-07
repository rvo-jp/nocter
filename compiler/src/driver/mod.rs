use crate::analysis::analyze_compile_unit;
use crate::ast::AstEnvelope;
use crate::diagnostics::{Diagnostic, DiagnosticsEnvelope};
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::home::{resolve_nocter_home, validate_nocter_home};
use crate::lexer::{TokensEnvelope, lex};
use crate::parser::parse;
use crate::source::{SourceId, SourceMap};
use crate::target::{DEFAULT_TARGET, HOST};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Help,
    Version,
    Doctor,
    Build(PathBuf),
    Run(PathBuf),
    Check(PathBuf),
    CheckJson(PathBuf),
    Fmt { check: bool, file: PathBuf },
    Tokens(PathBuf),
    Ast(PathBuf),
    Lsp,
}

pub fn run_from_env() -> ExitCode {
    run(env::args_os())
}

pub fn run<I, S>(args: I) -> ExitCode
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();
    let rest: Vec<OsString> = args.collect();

    match parse_command(&rest) {
        Ok(Command::Help) => {
            print_usage();
            ExitCode::SUCCESS
        }
        Ok(Command::Version) => {
            println!("nocter {VERSION}");
            println!("host: {HOST}");
            println!("default target: {DEFAULT_TARGET}");
            ExitCode::SUCCESS
        }
        Ok(Command::Doctor) => run_doctor(),
        Ok(Command::Build(file)) => not_implemented("build", &file),
        Ok(Command::Run(file)) => not_implemented("run", &file),
        Ok(Command::Check(file)) => not_implemented("check", &file),
        Ok(Command::CheckJson(file)) => run_check_json(&file),
        Ok(Command::Fmt { check, file }) => {
            let mode = if check { "fmt --check" } else { "fmt" };
            not_implemented(mode, &file)
        }
        Ok(Command::Tokens(file)) => run_tokens_json(&file),
        Ok(Command::Ast(file)) => run_ast_json(&file),
        Ok(Command::Lsp) => {
            eprintln!("error: nocter lsp is not implemented yet");
            ExitCode::FAILURE
        }
        Err(message) => {
            eprintln!("error: {message}");
            eprintln!();
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn parse_command(args: &[OsString]) -> Result<Command, String> {
    if args.is_empty() {
        return Ok(Command::Help);
    }

    let command = args[0].to_string_lossy();
    match command.as_ref() {
        "-h" | "--help" | "help" => Ok(Command::Help),
        "--version" | "version" => expect_no_extra(args, Command::Version),
        "doctor" => expect_no_extra(args, Command::Doctor),
        "build" => parse_one_path(args, Command::Build),
        "run" => parse_one_path(args, Command::Run),
        "check" => parse_check(args),
        "fmt" => parse_fmt(args),
        "tokens" => parse_json_tool_command(args, Command::Tokens),
        "ast" => parse_json_tool_command(args, Command::Ast),
        "lsp" => expect_no_extra(args, Command::Lsp),
        value if value.ends_with(".nct") => {
            expect_no_extra(args, Command::Run(PathBuf::from(args[0].clone())))
        }
        _ => Err(format!("unknown command `{command}`")),
    }
}

fn expect_no_extra(args: &[OsString], command: Command) -> Result<Command, String> {
    if args.len() == 1 {
        Ok(command)
    } else {
        Err(format!(
            "unexpected argument `{}`",
            args[1].to_string_lossy()
        ))
    }
}

fn parse_one_path(
    args: &[OsString],
    make_command: fn(PathBuf) -> Command,
) -> Result<Command, String> {
    match args {
        [_, file] => Ok(make_command(PathBuf::from(file.clone()))),
        [_] => Err("missing source file".to_string()),
        [_, _, extra, ..] => Err(format!("unexpected argument `{}`", extra.to_string_lossy())),
        [] => unreachable!("parse_one_path requires a command"),
    }
}

fn parse_fmt(args: &[OsString]) -> Result<Command, String> {
    match args {
        [_, flag] if flag.to_string_lossy() == "--check" => Err("missing source file".to_string()),
        [_, flag, file] if flag.to_string_lossy() == "--check" => Ok(Command::Fmt {
            check: true,
            file: PathBuf::from(file.clone()),
        }),
        [_, file] => Ok(Command::Fmt {
            check: false,
            file: PathBuf::from(file.clone()),
        }),
        [_] => Err("missing source file".to_string()),
        [_, extra, ..] => Err(format!("unexpected argument `{}`", extra.to_string_lossy())),
        [] => unreachable!("parse_fmt requires a command"),
    }
}

fn parse_check(args: &[OsString]) -> Result<Command, String> {
    match args {
        [_, file] => Ok(Command::Check(PathBuf::from(file.clone()))),
        [_] => Err("missing source file".to_string()),
        [_, file, flag, format] if is_arg(flag, "--format") && is_arg(format, "json") => {
            Ok(Command::CheckJson(PathBuf::from(file.clone())))
        }
        [_, _, flag] if is_arg(flag, "--format") => Err("expected `--format json`".to_string()),
        [_, _, flag, ..] if is_arg(flag, "--format") => Err("expected `--format json`".to_string()),
        [_, _, extra, ..] => Err(format!("unexpected argument `{}`", extra.to_string_lossy())),
        [] => unreachable!("parse_check requires a command"),
    }
}

fn parse_json_tool_command(
    args: &[OsString],
    make_command: fn(PathBuf) -> Command,
) -> Result<Command, String> {
    if args.is_empty() {
        unreachable!("parse_json_tool_command requires a command");
    }

    if args.len() == 1 {
        return Err("missing source file".to_string());
    }

    if args.len() == 2 {
        return Err("missing `--format json`".to_string());
    }

    if !is_arg(&args[2], "--format") {
        return Err(format!(
            "unexpected argument `{}`",
            args[2].to_string_lossy()
        ));
    }

    if args.len() == 3 {
        return Err("expected `--format json`".to_string());
    }

    if !is_arg(&args[3], "json") {
        return Err("expected `--format json`".to_string());
    }

    if args.len() > 4 {
        return Err(format!(
            "unexpected argument `{}`",
            args[4].to_string_lossy()
        ));
    }

    Ok(make_command(PathBuf::from(args[1].clone())))
}

fn is_arg(arg: &OsString, expected: &str) -> bool {
    arg.to_string_lossy() == expected
}

fn run_doctor() -> ExitCode {
    match resolve_nocter_home() {
        Ok(home) => {
            println!("Nocter home: {}", home.display());
            let errors = validate_nocter_home(&home);
            if errors.is_empty() {
                println!("ok");
                ExitCode::SUCCESS
            } else {
                for error in errors {
                    eprintln!("error: {error}");
                }
                install_error()
            }
        }
        Err(message) => {
            eprintln!("error: {message}");
            install_error()
        }
    }
}

fn run_tokens_json(file: &Path) -> ExitCode {
    let mut sources = SourceMap::new();

    let (envelope, status) = match sources.load_file(file) {
        Ok(source) => {
            let output = lex(&sources, source);
            let status = if output.diagnostics.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };

            match output.to_json_envelope(&sources, source) {
                Ok(envelope) => (envelope, status),
                Err(error) => {
                    eprintln!("internal compiler error: {error}");
                    return internal_error();
                }
            }
        }
        Err(diagnostic) => {
            let envelope = TokensEnvelope::new(
                file.to_string_lossy().into_owned(),
                canonical_absolute_string(file),
                Vec::new(),
                vec![diagnostic],
            );
            (envelope, ExitCode::FAILURE)
        }
    };

    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => {
            println!("{json}");
            status
        }
        Err(error) => {
            eprintln!("internal compiler error: failed to serialize token JSON: {error}");
            internal_error()
        }
    }
}

fn run_ast_json(file: &Path) -> ExitCode {
    let mut sources = SourceMap::new();

    let (envelope, status) = match sources.load_file(file) {
        Ok(source) => {
            let lexed = lex(&sources, source);
            if !lexed.diagnostics.is_empty() {
                let diagnostics = lexed.diagnostics;
                let envelope = AstEnvelope::new(
                    file.to_string_lossy().into_owned(),
                    canonical_absolute_string(file),
                    None,
                    diagnostics,
                );
                (envelope, ExitCode::FAILURE)
            } else {
                let parsed = parse(&sources, source, &lexed.tokens);
                let status = if parsed.diagnostics.is_empty() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                };
                let file = sources
                    .get(source)
                    .expect("loaded source id must resolve in source map");
                let envelope = AstEnvelope::new(
                    file.display_path().to_string(),
                    file.absolute_path()
                        .map(|path| path.to_string_lossy().into_owned()),
                    parsed.ast.map(|ast| ast.to_json(&sources)),
                    parsed.diagnostics,
                );
                (envelope, status)
            }
        }
        Err(diagnostic) => {
            let envelope = AstEnvelope::new(
                file.to_string_lossy().into_owned(),
                canonical_absolute_string(file),
                None,
                vec![diagnostic],
            );
            (envelope, ExitCode::FAILURE)
        }
    };

    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => {
            println!("{json}");
            status
        }
        Err(error) => {
            eprintln!("internal compiler error: failed to serialize AST JSON: {error}");
            internal_error()
        }
    }
}

fn run_check_json(file: &Path) -> ExitCode {
    let mut sources = SourceMap::new();

    let (envelope, status) = match sources.load_file(file) {
        Ok(source) => {
            let source_file = sources
                .get(source)
                .expect("loaded source id must resolve in source map");
            let root = source_file.display_path().to_string();
            let root_absolute_path = source_file
                .absolute_path()
                .map(|path| path.to_string_lossy().into_owned());
            let diagnostics = run_frontend_check(&mut sources, source);
            let status = if diagnostics.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            (
                DiagnosticsEnvelope::new(
                    "check",
                    None,
                    Some(root),
                    root_absolute_path,
                    diagnostics,
                ),
                status,
            )
        }
        Err(diagnostic) => {
            let envelope = DiagnosticsEnvelope::new(
                "check",
                None,
                Some(file.to_string_lossy().into_owned()),
                canonical_absolute_string(file),
                vec![diagnostic],
            );
            (envelope, ExitCode::FAILURE)
        }
    };

    match serde_json::to_string_pretty(&envelope) {
        Ok(json) => {
            println!("{json}");
            status
        }
        Err(error) => {
            eprintln!("internal compiler error: failed to serialize diagnostics JSON: {error}");
            internal_error()
        }
    }
}

fn run_frontend_check(sources: &mut SourceMap, source: crate::source::SourceId) -> Vec<Diagnostic> {
    run_frontend_check_with_options(sources, source, &FrontendOptions::default())
}

fn run_frontend_check_with_options(
    sources: &mut SourceMap,
    source: SourceId,
    options: &FrontendOptions,
) -> Vec<Diagnostic> {
    let unit = match load_compile_unit(sources, source, options) {
        Ok(unit) => unit,
        Err(diagnostics) => return diagnostics,
    };

    analyze_compile_unit(sources, &unit).diagnostics()
}

fn canonical_absolute_string(path: &Path) -> Option<String> {
    path.canonicalize()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

fn install_error() -> ExitCode {
    ExitCode::from(2)
}

fn internal_error() -> ExitCode {
    ExitCode::from(3)
}

fn not_implemented(command: &str, file: &Path) -> ExitCode {
    eprintln!(
        "error: nocter {command} is not implemented yet for `{}`",
        file.display()
    );
    ExitCode::FAILURE
}

fn print_usage() {
    println!("usage: nocter <command> [args]");
    println!();
    println!("commands:");
    println!("  build <file.nct>");
    println!("  run <file.nct>");
    println!("  <file.nct>");
    println!("  check <file.nct>");
    println!("  check <file.nct> --format json");
    println!("  fmt [--check] <file.nct>");
    println!("  tokens <file.nct> --format json");
    println!("  ast <file.nct> --format json");
    println!("  doctor");
    println!("  --version");
    println!("  lsp");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_project(name: &str) -> PathBuf {
        let unique = format!(
            "nocter-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = env::temp_dir().join(unique);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn make_nocter_home(root: &Path) -> PathBuf {
        let home = root.join(".nocter");
        fs::create_dir_all(home.join("std")).unwrap();
        fs::create_dir_all(home.join("targets/arm64-darwin/std")).unwrap();
        fs::write(home.join("std/prelude.nct"), "pub type Int = i32\n").unwrap();
        home
    }

    fn check_with_nocter_home(
        sources: &mut SourceMap,
        source: SourceId,
        home: &Path,
    ) -> Vec<Diagnostic> {
        run_frontend_check_with_options(
            sources,
            source,
            &FrontendOptions {
                nocter_home: Some(home.to_path_buf()),
                target: DEFAULT_TARGET.to_string(),
            },
        )
    }

    #[test]
    fn compile_unit_analysis_retains_per_file_results() {
        let root = make_temp_project("compile-unit-analysis");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./config import answer

program(): i32 {
    return answer()
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("config.nct"),
            r#"pub func answer(): i32 {
    return "bad"
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let options = FrontendOptions {
            nocter_home: Some(home.to_path_buf()),
            target: DEFAULT_TARGET.to_string(),
        };
        let unit = load_compile_unit(&mut sources, source, &options).unwrap();
        let analysis = analyze_compile_unit(&sources, &unit);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(analysis.files.iter().filter(|file| file.is_root).count(), 1);
        assert!(analysis.files.iter().any(|file| {
            file.is_root && file.ast.span.source == source && file.diagnostics.is_empty()
        }));

        let config = analysis
            .files
            .iter()
            .find(|file| {
                sources
                    .get(file.ast.span.source)
                    .and_then(|source_file| source_file.absolute_path())
                    .map(|path| path.ends_with("config.nct"))
                    .unwrap_or(false)
            })
            .expect("expected config.nct analysis");
        assert!(config.resolved.symbols.symbol_by_name("answer").is_some());
        assert!(
            config
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E0312")
        );

        assert_eq!(
            analysis
                .diagnostics()
                .iter()
                .filter(|diagnostic| diagnostic.code == "E0312")
                .count(),
            1
        );
    }

    #[test]
    fn check_orders_diagnostics_by_source_position() {
        let root = make_temp_project("diagnostic-order");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"func takes_i32(value: i32): i32 {
    return value
}

program(): i32 {
    return "bad"
}

func later(): i32 {
    return takes_i32("bad")
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0312");
        assert_eq!(diagnostics[1].code, "E0321");

        let first_span = diagnostics[0].primary_span.as_ref().unwrap();
        let second_span = diagnostics[1].primary_span.as_ref().unwrap();
        assert!(first_span.start_byte < second_span.start_byte);
    }

    #[test]
    fn parses_bare_source_as_run() {
        let command = parse_command(&[OsString::from("app.nct")]).unwrap();
        assert_eq!(command, Command::Run(PathBuf::from("app.nct")));
    }

    #[test]
    fn parses_fmt_check() {
        let command = parse_command(&[
            OsString::from("fmt"),
            OsString::from("--check"),
            OsString::from("app.nct"),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::Fmt {
                check: true,
                file: PathBuf::from("app.nct")
            }
        );
    }

    #[test]
    fn rejects_fmt_check_without_file() {
        let error = parse_command(&[OsString::from("fmt"), OsString::from("--check")]).unwrap_err();
        assert_eq!(error, "missing source file");
    }

    #[test]
    fn parses_tokens_json() {
        let command = parse_command(&[
            OsString::from("tokens"),
            OsString::from("app.nct"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap();

        assert_eq!(command, Command::Tokens(PathBuf::from("app.nct")));
    }

    #[test]
    fn parses_check_json() {
        let command = parse_command(&[
            OsString::from("check"),
            OsString::from("app.nct"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap();

        assert_eq!(command, Command::CheckJson(PathBuf::from("app.nct")));
    }

    #[test]
    fn parses_ast_json() {
        let command = parse_command(&[
            OsString::from("ast"),
            OsString::from("app.nct"),
            OsString::from("--format"),
            OsString::from("json"),
        ])
        .unwrap();

        assert_eq!(command, Command::Ast(PathBuf::from("app.nct")));
    }

    #[test]
    fn rejects_tokens_without_json_format() {
        let error =
            parse_command(&[OsString::from("tokens"), OsString::from("app.nct")]).unwrap_err();
        assert_eq!(error, "missing `--format json`");
    }

    #[test]
    fn validates_nocter_home_shape() {
        let root = make_temp_project("home-shape");
        fs::create_dir_all(root.join("std")).unwrap();
        fs::create_dir_all(root.join("targets/arm64-darwin/std")).unwrap();
        fs::write(root.join("VERSION"), "0.1.0\n").unwrap();
        fs::write(
            root.join("MANIFEST.json"),
            r#"{
  "schema": "nocter.manifest",
  "schema_version": 1,
  "release": "0.1.0",
  "host": "arm64-darwin",
  "default_target": "arm64-darwin",
  "compiler": {
    "path": "nocter"
  },
  "std": {
    "path": "std"
  },
  "implemented_targets": [
    {
      "name": "arm64-darwin",
      "std_path": "targets/arm64-darwin/std",
      "backend": "arm64",
      "executable": "macho",
      "os": "darwin"
    }
  ],
  "archive": {
    "name": "nocter-v0.1.0-arm64-darwin.tar.gz",
    "root": ".nocter"
  }
}
"#,
        )
        .unwrap();

        let errors = validate_nocter_home(&root);
        fs::remove_dir_all(&root).unwrap();

        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn check_loads_relative_imports() {
        let root = make_temp_project("relative-import");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./config import answer

program(): i32 {
    return answer()
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("config.nct"),
            r#"pub func answer(): i32 {
    return 1
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn check_uses_relative_imported_function_return_type() {
        let root = make_temp_project("relative-import-return-type");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./config import title

program(): i32 {
    return title()
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("config.nct"),
            r#"pub func title(): str {
    return "Nocter"
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn check_uses_relative_imported_function_parameters() {
        let root = make_temp_project("relative-import-parameters");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./config import answer

program(): i32 {
    return answer()
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("config.nct"),
            r#"pub func answer(value: i32): i32 {
    return value
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0320");
    }

    #[test]
    fn check_uses_relative_imported_associated_function_return_type() {
        let root = make_temp_project("relative-import-associated-function");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./geometry import Point

program(): i32 {
    return Point.origin().x
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("geometry.nct"),
            r#"pub struct Point {
    pub x: i32
}

impl Point {
    pub func origin(): Point {
        return Point{ x: 0 }
    }
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn check_uses_relative_imported_method_return_type() {
        let root = make_temp_project("relative-import-method");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./geometry import Point

program(): i32 {
    let point = Point.origin()
    return point.x_value()
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("geometry.nct"),
            r#"pub struct Point {
    pub x: i32
}

impl Point {
    pub func origin(): Point {
        return Point{ x: 0 }
    }

    pub method (point: Self).x_value(): i32 {
        return point.x
    }
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn check_reports_relative_imported_function_body_errors() {
        let root = make_temp_project("relative-import-function-body-error");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./config import answer

program(): i32 {
    return 0
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("config.nct"),
            r#"pub func answer(): i32 {
    return "bad"
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("i32"));
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn check_reports_relative_imported_impl_member_name_duplicates() {
        let root = make_temp_project("relative-import-impl-duplicate");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./geometry import Point

program(): i32 {
    return 0
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("geometry.nct"),
            r#"pub struct Point {
    pub x: i32
}

impl Point {
    pub func origin(): Point {
        return Point{ x: 0 }
    }

    pub method (point: Self).origin(): i32 {
        return point.x
    }
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].code, "E0413");
    }

    #[test]
    fn check_reports_missing_relative_imported_names() {
        let root = make_temp_project("missing-relative-imported-name");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./config import missing

program(): i32 {
    return 0
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("config.nct"),
            r#"func answer(): i32 {
    return 1
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0411");
    }

    #[test]
    fn check_reports_private_relative_imported_names() {
        let root = make_temp_project("private-relative-imported-name");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./config import answer

program(): i32 {
    return 0
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("config.nct"),
            r#"func answer(): i32 {
    return 1
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0412");
        assert!(diagnostics[0].message.contains("private"));
    }

    #[test]
    fn check_reports_relative_import_parse_errors() {
        let root = make_temp_project("relative-import-parse-error");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./config import answer

program(): i32 {
    return 0
}
"#,
        )
        .unwrap();
        fs::write(root.join("config.nct"), "module config\n").unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0200");
    }

    #[test]
    fn check_reports_missing_relative_imports() {
        let root = make_temp_project("missing-relative-import");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from ./missing import Missing

program(): i32 {
    return 0
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0410");
    }

    #[test]
    fn check_loads_non_relative_std_imports_from_nocter_home() {
        let root = make_temp_project("std-import");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from std/io import answer

program(): i32 {
    return answer()
}
"#,
        )
        .unwrap();
        fs::write(
            home.join("std/io.nct"),
            r#"pub func answer(): i32 {
    return 1
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn check_loads_namespace_imports_from_nocter_home() {
        let root = make_temp_project("std-namespace-import");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"import std/io as io

program(): i32 {
    return 0
}
"#,
        )
        .unwrap();
        fs::write(
            home.join("std/io.nct"),
            r#"pub func answer(): i32 {
    return 1
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn check_uses_non_relative_imported_function_return_type() {
        let root = make_temp_project("std-import-return-type");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from std/io import title

program(): i32 {
    return title()
}
"#,
        )
        .unwrap();
        fs::write(
            home.join("std/io.nct"),
            r#"pub func title(): str {
    return "Nocter"
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn check_prefers_target_overlay_for_std_imports() {
        let root = make_temp_project("std-import-overlay");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from std/io import answer

program(): i32 {
    return answer()
}
"#,
        )
        .unwrap();
        fs::write(
            home.join("std/io.nct"),
            r#"pub func answer(): str {
    return "common"
}
"#,
        )
        .unwrap();
        fs::write(
            home.join("targets/arm64-darwin/std/io.nct"),
            r#"pub func answer(): i32 {
    return 1
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn check_synthesizes_standard_prelude_for_user_modules() {
        let root = make_temp_project("synthetic-prelude");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"program(): i32 {
    return answer()
}
"#,
        )
        .unwrap();
        fs::write(
            home.join("std/prelude.nct"),
            r#"pub from std/prelude_helpers import answer
"#,
        )
        .unwrap();
        fs::write(
            home.join("std/prelude_helpers.nct"),
            r#"pub func answer(value: i32): i32 {
    return value
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0320");
    }

    #[test]
    fn check_accepts_builtin_str_return_type() {
        let root = make_temp_project("builtin-str-return");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"program(): i32 {
    return 0
}

func title(): str {
    return "Nocter"
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn check_diagnoses_mismatched_builtin_str_return_type() {
        let root = make_temp_project("builtin-str-return-mismatch");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"program(): i32 {
    return 0
}

func title(): str {
    return 1
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0312");
        assert!(diagnostics[0].message.contains("str"));
    }

    #[test]
    fn check_reports_nocter_visibility_import_from_user_project() {
        let root = make_temp_project("nocter-visibility-user-import");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from std/ptr import internal

program(): i32 {
    return 0
}
"#,
        )
        .unwrap();
        fs::write(
            home.join("std/ptr.nct"),
            r#"pub(nocter) func internal(): i32 {
    return 1
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0412");
        assert!(diagnostics[0].message.contains("pub(nocter)"));
    }

    #[test]
    fn check_allows_nocter_visibility_import_inside_nocter_home() {
        let root = make_temp_project("nocter-visibility-home-import");
        let home = make_nocter_home(&root);
        fs::write(
            home.join("std/io.nct"),
            r#"from std/ptr import internal

program(): i32 {
    return internal()
}
"#,
        )
        .unwrap();
        fs::write(
            home.join("std/ptr.nct"),
            r#"pub(nocter) func internal(): i32 {
    return 1
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(home.join("std/io.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn check_reports_missing_non_relative_imports() {
        let root = make_temp_project("missing-std-import");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"from std/missing import answer

program(): i32 {
    return 0
}
"#,
        )
        .unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0410");
    }

    #[test]
    fn check_loads_non_relative_use_imports() {
        let root = make_temp_project("std-use");
        let home = make_nocter_home(&root);
        fs::write(
            root.join("app.nct"),
            r#"use std/prelude

program(): i32 {
    return 0
}
"#,
        )
        .unwrap();
        fs::write(home.join("std/prelude.nct"), "module prelude\n").unwrap();

        let mut sources = SourceMap::new();
        let source = sources.load_file(root.join("app.nct")).unwrap();
        let diagnostics = check_with_nocter_home(&mut sources, source, &home);
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "E0200");
    }
}
