mod manifest;

use crate::ast::AstEnvelope;
use crate::diagnostics::{Diagnostic, DiagnosticsEnvelope};
use crate::lexer::{TokensEnvelope, lex};
use crate::parser::parse;
use crate::resolve::resolve;
use crate::source::SourceMap;
use crate::typecheck::check;
use manifest::Manifest;
use std::collections::HashSet;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const HOST: &str = "arm64-darwin";
pub const DEFAULT_TARGET: &str = HOST;
const MANIFEST_SCHEMA: &str = "nocter.manifest";
const MANIFEST_SCHEMA_VERSION: u32 = 1;

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
            let diagnostics = run_frontend_check(&sources, source);
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

fn run_frontend_check(sources: &SourceMap, source: crate::source::SourceId) -> Vec<Diagnostic> {
    let lexed = lex(sources, source);
    if !lexed.diagnostics.is_empty() {
        return lexed.diagnostics;
    }

    let parsed = parse(sources, source, &lexed.tokens);
    if !parsed.diagnostics.is_empty() {
        return parsed.diagnostics;
    }

    let Some(ast) = parsed.ast else {
        return vec![Diagnostic::error(
            "E0200",
            "parser did not produce an AST and did not report a diagnostic",
        )];
    };

    let resolved = resolve(sources, &ast);
    let mut diagnostics = resolved.diagnostics.clone();
    diagnostics.extend(check(sources, &ast, &resolved));
    diagnostics
}

fn canonical_absolute_string(path: &Path) -> Option<String> {
    path.canonicalize()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
}

fn resolve_nocter_home() -> Result<PathBuf, String> {
    if let Some(home) = env::var_os("NOCTER_HOME") {
        return Ok(PathBuf::from(home));
    }

    let exe = env::current_exe()
        .map_err(|error| format!("failed to resolve running nocter executable: {error}"))?;
    let resolved = exe
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize running nocter executable: {error}"))?;
    resolved
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "running nocter executable has no parent directory".to_string())
}

fn validate_nocter_home(home: &Path) -> Vec<String> {
    let mut errors = Vec::new();

    if !home.is_dir() {
        errors.push(format!(
            "Nocter home is not a directory `{}`",
            home.display()
        ));
        return errors;
    }

    let version = match read_version_file(&home.join("VERSION")) {
        Ok(version) => Some(version),
        Err(error) => {
            errors.push(error);
            None
        }
    };

    let manifest = match manifest::load_manifest(&home.join("MANIFEST.json")) {
        Ok(manifest) => Some(manifest),
        Err(error) => {
            errors.push(error);
            None
        }
    };

    require_dir(home, "std", &mut errors);
    require_dir(home, "targets", &mut errors);

    if let (Some(version), Some(manifest)) = (version.as_deref(), manifest.as_ref()) {
        validate_manifest(home, version, manifest, &mut errors);
    }

    errors
}

fn require_dir(home: &Path, relative: &str, errors: &mut Vec<String>) {
    let path = home.join(relative);
    if !path.is_dir() {
        errors.push(format!("missing directory `{}`", path.display()));
    }
}

fn install_error() -> ExitCode {
    ExitCode::from(2)
}

fn internal_error() -> ExitCode {
    ExitCode::from(3)
}

fn read_version_file(path: &Path) -> Result<String, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read `{}`: {error}", path.display()))?;
    let mut lines = text.lines();
    let Some(version) = lines.next() else {
        return Err(format!("`{}` is empty", path.display()));
    };

    if lines.next().is_some() {
        return Err(format!(
            "`{}` must contain exactly one line",
            path.display()
        ));
    }

    if version.trim() != version {
        return Err(format!(
            "`{}` must not contain leading or trailing whitespace",
            path.display()
        ));
    }

    if !is_valid_release_version(version) {
        return Err(format!(
            "`{}` contains invalid release version `{version}`",
            path.display()
        ));
    }

    Ok(version.to_string())
}

fn is_valid_release_version(version: &str) -> bool {
    if version.is_empty() || version.starts_with('v') {
        return false;
    }

    let (core, prerelease) = match version.split_once('-') {
        Some((core, prerelease)) => (core, Some(prerelease)),
        None => (version, None),
    };

    let mut parts = core.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let Some(patch) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }

    let numeric = [major, minor, patch]
        .into_iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
    if !numeric {
        return false;
    }

    match prerelease {
        Some(part) => {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
        }
        None => true,
    }
}

fn validate_manifest(home: &Path, version: &str, manifest: &Manifest, errors: &mut Vec<String>) {
    if manifest.schema != MANIFEST_SCHEMA {
        errors.push(format!(
            "MANIFEST.json schema must be `{MANIFEST_SCHEMA}`, got `{}`",
            manifest.schema
        ));
    }

    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        errors.push(format!(
            "MANIFEST.json schema_version must be {MANIFEST_SCHEMA_VERSION}, got {}",
            manifest.schema_version
        ));
    }

    if manifest.release != version {
        errors.push(format!(
            "MANIFEST.json release `{}` does not match VERSION `{version}`",
            manifest.release
        ));
    }

    if manifest.host != HOST {
        errors.push(format!(
            "MANIFEST.json host must be `{HOST}`, got `{}`",
            manifest.host
        ));
    }

    if manifest.default_target != DEFAULT_TARGET {
        errors.push(format!(
            "MANIFEST.json default_target must be `{DEFAULT_TARGET}`, got `{}`",
            manifest.default_target
        ));
    }

    if manifest.compiler.path.as_path() != Path::new("nocter") {
        errors.push("MANIFEST.json compiler.path must be `nocter`".to_string());
    }
    validate_relative_path("compiler.path", &manifest.compiler.path, errors);

    if manifest.std.path.as_path() != Path::new("std") {
        errors.push("MANIFEST.json std.path must be `std`".to_string());
    }
    validate_relative_path("std.path", &manifest.std.path, errors);
    if !home.join(&manifest.std.path).is_dir() {
        errors.push(format!(
            "std.path directory is missing `{}`",
            home.join(&manifest.std.path).display()
        ));
    }

    let mut names = HashSet::new();
    for target in &manifest.implemented_targets {
        if !names.insert(target.name.as_str()) {
            errors.push(format!("duplicate implemented target `{}`", target.name));
        }

        if target.name != HOST {
            errors.push(format!(
                "v0 supports only implemented target `{HOST}`, got `{}`",
                target.name
            ));
        }

        if target.name == HOST {
            if target.backend != "arm64" {
                errors.push(format!(
                    "target `{HOST}` backend must be `arm64`, got `{}`",
                    target.backend
                ));
            }
            if target.executable != "macho" {
                errors.push(format!(
                    "target `{HOST}` executable must be `macho`, got `{}`",
                    target.executable
                ));
            }
            if target.os != "darwin" {
                errors.push(format!(
                    "target `{HOST}` os must be `darwin`, got `{}`",
                    target.os
                ));
            }
        }

        validate_relative_path("implemented_targets[].std_path", &target.std_path, errors);
        if !home.join(&target.std_path).is_dir() {
            errors.push(format!(
                "target std_path directory is missing `{}`",
                home.join(&target.std_path).display()
            ));
        }
    }

    if !names.contains(manifest.default_target.as_str()) {
        errors.push(format!(
            "default_target `{}` is not listed in implemented_targets",
            manifest.default_target
        ));
    }

    if manifest.archive.name != format!("nocter-v{version}-{HOST}.tar.gz") {
        errors.push(format!(
            "archive.name must be `nocter-v{version}-{HOST}.tar.gz`, got `{}`",
            manifest.archive.name
        ));
    }

    if manifest.archive.root.as_path() != Path::new(".nocter") {
        errors.push("archive.root must be `.nocter`".to_string());
    }
    validate_relative_path("archive.root", &manifest.archive.root, errors);
}

fn validate_relative_path(label: &str, path: &Path, errors: &mut Vec<String>) {
    if path.as_os_str().is_empty() {
        errors.push(format!("MANIFEST.json {label} must not be empty"));
        return;
    }

    if path.is_absolute() {
        errors.push(format!("MANIFEST.json {label} must be relative"));
        return;
    }

    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        errors.push(format!("MANIFEST.json {label} must not contain `..`"));
    }
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
        let unique = format!(
            "nocter-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = env::temp_dir().join(unique);
        std::fs::create_dir_all(root.join("std")).unwrap();
        std::fs::create_dir_all(root.join("targets/arm64-darwin/std")).unwrap();
        std::fs::write(root.join("VERSION"), "0.1.0\n").unwrap();
        std::fs::write(
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
        std::fs::remove_dir_all(&root).unwrap();

        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn rejects_invalid_version_prefix() {
        assert!(!is_valid_release_version("v0.1.0"));
    }

    #[test]
    fn accepts_prerelease_version() {
        assert!(is_valid_release_version("0.1.0-dev"));
    }
}
