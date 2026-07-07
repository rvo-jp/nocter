use super::*;
use crate::analysis::{CompileUnit, analyze_compile_unit};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::ir::{Function, Instruction, IrModule, Type};
use crate::source::SourceMap;
use crate::target::DEFAULT_TARGET;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn lowers_program_returning_i32_literal() {
    let ir = lower_text(
        r#"program(): i32 {
    return 42
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::ReturnI32(42)],
        }])
    );
}

#[test]
fn lowers_program_returning_negative_i32_literal() {
    let ir = lower_text(
        r#"program(): i32 {
    return -42
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![Instruction::ReturnI32(-42)]
    );
}

#[test]
fn lowers_fallible_program_returning_i32_literal() {
    let ir = lower_text(
        r#"program(): i32! {
    return 7
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![Instruction::ReturnI32(7)],
        }])
    );
}

#[test]
fn lowers_fallible_program_fail_make_error() {
    let ir = lower_text(
        r#"primitive make_error(code: str, message: str): error

program(): i32! {
    fail make_error("app.failed", "failed")
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::Fallible(Box::new(Type::I32)),
            instructions: vec![
                Instruction::WriteStaticStderr(b"failed\n".to_vec()),
                Instruction::ReturnI32(1),
            ],
        }])
    );
}

#[test]
fn lowers_fallible_program_fail_message_without_duplicate_newline() {
    let ir = lower_text(
        r#"primitive make_error(code: str, message: str): error

program(): i32! {
    fail make_error("app.failed", "failed\n")
}
"#,
    );

    assert_eq!(
        ir.functions[0].instructions,
        vec![
            Instruction::WriteStaticStderr(b"failed\n".to_vec()),
            Instruction::ReturnI32(1),
        ]
    );
}

#[test]
fn reports_unsupported_fail_payload() {
    let diagnostics = lower_text_diagnostics(
        r#"primitive make_error(code: str, message: str): error

program(): i32! {
    fail make_error("app.failed", dynamic())
}

func dynamic(): str {
    return "failed"
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8004");
}

#[test]
fn lowers_void_program_with_empty_body() {
    let ir = lower_text(
        r#"program(): void {
}
"#,
    );

    assert_eq!(
        ir,
        IrModule::new(vec![Function {
            name: "program".to_string(),
            return_type: Type::Void,
            instructions: vec![Instruction::ReturnVoid],
        }])
    );
}

#[test]
fn reports_unsupported_program_body() {
    let diagnostics = lower_text_diagnostics(
        r#"program(): i32 {
    let value = 1
    return value
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8002");
}

#[test]
fn rejects_nested_negative_integer_literal() {
    let diagnostics = lower_text_diagnostics(
        r#"program(): i32 {
    return -(-42)
}
"#,
    );

    assert_eq!(diagnostics[0].code, "E8003");
}

fn lower_text(text: &str) -> IrModule {
    let diagnostics = lower_text_diagnostics(text);
    match diagnostics.as_slice() {
        [] => {
            let analysis = analyze_text(text);
            lower_program(&analysis).unwrap()
        }
        diagnostics => panic!("unexpected diagnostics: {diagnostics:?}"),
    }
}

fn lower_text_diagnostics(text: &str) -> Vec<Diagnostic> {
    let analysis = analyze_text(text);
    match lower_program(&analysis) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn analyze_text(text: &str) -> crate::analysis::CompileUnitAnalysis {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let temp_root = make_temp_project();
    let nocter_home = make_nocter_home(&temp_root);
    let unit: CompileUnit = load_compile_unit(
        &mut sources,
        source,
        &FrontendOptions {
            nocter_home: Some(nocter_home),
            target: DEFAULT_TARGET.to_string(),
        },
    )
    .unwrap();
    let analysis = analyze_compile_unit(&sources, &unit);
    let diagnostics = analysis.diagnostics();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    analysis
}

fn make_temp_project() -> PathBuf {
    let unique = format!(
        "nocter-ir-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).unwrap();
    root
}

fn make_nocter_home(root: &Path) -> PathBuf {
    let home = root.join(".nocter");
    fs::create_dir_all(home.join("std")).unwrap();
    fs::create_dir_all(home.join("targets/arm64-darwin/std")).unwrap();
    fs::write(home.join("std/prelude.nct"), "").unwrap();
    home
}
