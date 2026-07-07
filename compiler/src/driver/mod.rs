mod manifest;

use crate::analysis::{CompileUnit, analyze_compile_unit};
use crate::ast::AstEnvelope;
use crate::diagnostics::{Diagnostic, DiagnosticsEnvelope};
use crate::lexer::{TokensEnvelope, lex};
use crate::parser::parse;
use crate::resolve::{ImportAccess, ImportSource, ImportSourceMap};
use crate::source::{ByteSpan, SourceId, SourceMap};
use manifest::Manifest;
use std::collections::{HashSet, VecDeque};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const HOST: &str = "arm64-darwin";
pub const DEFAULT_TARGET: &str = HOST;
const STANDARD_PRELUDE_PATH: &str = "std/prelude";
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

#[derive(Debug, Clone)]
struct FrontendOptions {
    nocter_home: Option<PathBuf>,
    target: String,
}

impl Default for FrontendOptions {
    fn default() -> Self {
        Self {
            nocter_home: None,
            target: DEFAULT_TARGET.to_string(),
        }
    }
}

fn load_compile_unit(
    sources: &mut SourceMap,
    root: SourceId,
    options: &FrontendOptions,
) -> Result<CompileUnit, Vec<Diagnostic>> {
    let mut queue = VecDeque::from([root]);
    let mut queued_sources = HashSet::from([root]);
    let mut loaded_sources_by_path = std::collections::HashMap::new();
    let mut import_sources = ImportSourceMap::new();
    let mut resolved_nocter_home = None;
    let mut diagnostics = Vec::new();
    let mut root_ast = None;
    let mut files = Vec::new();

    if let Some(path) = sources
        .get(root)
        .and_then(|file| file.absolute_path())
        .cloned()
    {
        loaded_sources_by_path.insert(path, root);
    }

    while let Some(source) = queue.pop_front() {
        let mut ast = match parse_source_for_check(sources, source) {
            Ok(ast) => ast,
            Err(source_diagnostics) => {
                diagnostics.extend(source_diagnostics);
                continue;
            }
        };

        if should_synthesize_prelude(sources, source, &ast, options, &mut resolved_nocter_home) {
            synthesize_prelude_use(source, &mut ast);
        }

        if source == root {
            root_ast = Some(ast.clone());
        }
        files.push(ast.clone());

        for path in import_paths(&ast) {
            let canonical = match resolve_import_path(
                sources,
                source,
                path,
                options,
                &mut resolved_nocter_home,
            ) {
                Ok(path) => path,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };

            let imported = match loaded_sources_by_path.get(&canonical).copied() {
                Some(source) => source,
                None => match sources.load_file(&canonical) {
                    Ok(source) => {
                        loaded_sources_by_path.insert(canonical, source);
                        source
                    }
                    Err(error) => {
                        diagnostics.push(import_source_diagnostic(
                            sources,
                            path.span,
                            &path.value,
                            error,
                        ));
                        continue;
                    }
                },
            };

            import_sources.insert(
                path.span,
                ImportSource {
                    source: imported,
                    access: import_access_for_source(
                        sources,
                        source,
                        options,
                        &resolved_nocter_home,
                    ),
                },
            );

            if queued_sources.insert(imported) {
                queue.push_back(imported);
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let Some(root_ast) = root_ast else {
        return Err(vec![Diagnostic::error(
            "E0200",
            "root source did not produce an AST and did not report a diagnostic",
        )]);
    };

    Ok(CompileUnit::new(root_ast, files, import_sources))
}

fn parse_source_for_check(
    sources: &SourceMap,
    source: crate::source::SourceId,
) -> Result<crate::ast::AstFile, Vec<Diagnostic>> {
    let lexed = lex(sources, source);
    if !lexed.diagnostics.is_empty() {
        return Err(lexed.diagnostics);
    }

    let parsed = parse(sources, source, &lexed.tokens);
    if !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics);
    }

    let Some(ast) = parsed.ast else {
        return Err(vec![Diagnostic::error(
            "E0200",
            "parser did not produce an AST and did not report a diagnostic",
        )]);
    };

    Ok(ast)
}

fn import_paths(ast: &crate::ast::AstFile) -> Vec<&crate::ast::ModulePath> {
    ast.items
        .iter()
        .filter_map(|item| match item {
            crate::ast::Item::Use(item) => Some(&item.path),
            crate::ast::Item::Import(item) => Some(&item.path),
            crate::ast::Item::FromImport(item) => Some(&item.path),
            _ => None,
        })
        .collect()
}

fn should_synthesize_prelude(
    sources: &SourceMap,
    source: SourceId,
    ast: &crate::ast::AstFile,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> bool {
    if ast.items.iter().any(is_standard_prelude_use) {
        return false;
    }

    let Ok(home) = active_nocter_home(options, resolved_nocter_home) else {
        return true;
    };
    let home = canonicalize_existing(&home);
    let Some(source_path) = sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .map(|path| canonicalize_existing(path))
    else {
        return true;
    };

    !source_path.starts_with(home)
}

fn is_standard_prelude_use(item: &crate::ast::Item) -> bool {
    matches!(item, crate::ast::Item::Use(use_) if use_.path.value == STANDARD_PRELUDE_PATH)
}

fn synthesize_prelude_use(source: SourceId, ast: &mut crate::ast::AstFile) {
    let span = ByteSpan::new(source, 0, 0);
    ast.items.insert(
        0,
        crate::ast::Item::Use(crate::ast::UseItem {
            span,
            path: crate::ast::ModulePath {
                span,
                value: STANDARD_PRELUDE_PATH.to_string(),
                segments: vec!["std".to_string(), "prelude".to_string()],
            },
        }),
    );
}

fn is_relative_module_path(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../")
}

fn resolve_import_path(
    sources: &SourceMap,
    source: SourceId,
    path: &crate::ast::ModulePath,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> Result<PathBuf, Diagnostic> {
    if is_relative_module_path(&path.value) {
        let Some(resolved_path) = resolve_relative_import_path(sources, source, path) else {
            return Err(relative_import_without_file_path_diagnostic(
                sources, path.span,
            ));
        };

        return resolved_path.canonicalize().map_err(|error| {
            import_load_diagnostic(
                sources,
                path.span,
                &path.value,
                &[resolved_path],
                error,
                ImportPathKind::Relative,
            )
        });
    }

    let home = active_nocter_home(options, resolved_nocter_home).map_err(|message| {
        nocter_home_import_diagnostic(sources, path.span, &path.value, message)
    })?;
    let candidates = non_relative_import_candidates(&home, &options.target, &path.value);

    for candidate in &candidates {
        if let Ok(canonical) = candidate.canonicalize() {
            return Ok(canonical);
        }
    }

    Err(import_load_diagnostic(
        sources,
        path.span,
        &path.value,
        &candidates,
        "file was not found in any import root",
        ImportPathKind::NonRelative,
    ))
}

fn resolve_relative_import_path(
    sources: &SourceMap,
    source: SourceId,
    path: &crate::ast::ModulePath,
) -> Option<PathBuf> {
    let source_file = sources.get(source)?;
    let source_path = source_file.absolute_path()?;
    let source_dir = source_path.parent()?;
    Some(source_dir.join(format!("{}.nct", path.value)))
}

fn active_nocter_home(
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> Result<PathBuf, String> {
    if let Some(home) = &options.nocter_home {
        return Ok(home.clone());
    }

    if let Some(cached) = resolved_nocter_home {
        return cached.clone();
    }

    let resolved = resolve_nocter_home();
    *resolved_nocter_home = Some(resolved.clone());
    resolved
}

fn import_access_for_source(
    sources: &SourceMap,
    source: SourceId,
    options: &FrontendOptions,
    resolved_nocter_home: &Option<Result<PathBuf, String>>,
) -> ImportAccess {
    let Some(home) = current_nocter_home(options, resolved_nocter_home) else {
        return ImportAccess::Public;
    };
    let Some(source_path) = sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .map(|path| canonicalize_existing(path))
    else {
        return ImportAccess::Public;
    };

    if source_path.starts_with(home) {
        ImportAccess::Nocter
    } else {
        ImportAccess::Public
    }
}

fn current_nocter_home(
    options: &FrontendOptions,
    resolved_nocter_home: &Option<Result<PathBuf, String>>,
) -> Option<PathBuf> {
    if let Some(home) = &options.nocter_home {
        return Some(canonicalize_existing(home));
    }

    resolved_nocter_home
        .as_ref()
        .and_then(|home| home.as_ref().ok())
        .map(|home| canonicalize_existing(home))
}

fn canonicalize_existing(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn non_relative_import_candidates(home: &Path, target: &str, import_path: &str) -> Vec<PathBuf> {
    if let Some(std_path) = import_path.strip_prefix("std/") {
        return vec![
            home.join("targets")
                .join(target)
                .join("std")
                .join(format!("{std_path}.nct")),
            home.join("std").join(format!("{std_path}.nct")),
        ];
    }

    vec![home.join(format!("{import_path}.nct"))]
}

fn relative_import_without_file_path_diagnostic(
    sources: &SourceMap,
    span: crate::source::ByteSpan,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        "relative import cannot be resolved because the importing source has no file path",
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help =
        Some("load the root source from a file before resolving relative imports".to_string());
    diagnostic
}

fn import_load_diagnostic(
    sources: &SourceMap,
    span: crate::source::ByteSpan,
    import_path: &str,
    candidates: &[PathBuf],
    error: impl std::fmt::Display,
    kind: ImportPathKind,
) -> Diagnostic {
    let searched = candidates
        .iter()
        .map(|path| format!("`{}`", path.display()))
        .collect::<Vec<_>>()
        .join(", ");
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!("failed to resolve import `{import_path}`; searched {searched}: {error}"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(match kind {
        ImportPathKind::Relative => {
            "relative imports are resolved from the importing file directory and automatically add `.nct`"
                .to_string()
        }
        ImportPathKind::NonRelative => {
            "non-relative imports are resolved inside the active Nocter home; `std/...` searches the active target overlay before common `std/`"
                .to_string()
        }
    });
    diagnostic
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportPathKind {
    Relative,
    NonRelative,
}

fn nocter_home_import_diagnostic(
    sources: &SourceMap,
    span: crate::source::ByteSpan,
    import_path: &str,
    error: impl std::fmt::Display,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!("failed to resolve Nocter home while loading import `{import_path}`: {error}"),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = Some(
        "set `NOCTER_HOME` to the active Nocter home, or run the `nocter` binary from inside its installed `.nocter/` directory"
            .to_string(),
    );
    diagnostic
}

fn import_source_diagnostic(
    sources: &SourceMap,
    span: crate::source::ByteSpan,
    import_path: &str,
    source_error: Diagnostic,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0410",
        format!(
            "failed to load import `{import_path}`: {}",
            source_error.message
        ),
    );
    diagnostic.primary_span = sources.span_to_json(span).ok().map(Box::new);
    diagnostic.help = source_error.help;
    diagnostic
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

    #[test]
    fn rejects_invalid_version_prefix() {
        assert!(!is_valid_release_version("v0.1.0"));
    }

    #[test]
    fn accepts_prerelease_version() {
        assert!(is_valid_release_version("0.1.0-dev"));
    }
}
