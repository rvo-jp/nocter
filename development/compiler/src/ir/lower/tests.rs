use super::*;
use crate::abi::{ReturnPassing, ValueLayout};
use crate::analysis::{CompileUnit, CompileUnitAnalysis, analyze_executable_compile_unit};
use crate::diagnostics::Diagnostic;
use crate::frontend::{FrontendOptions, load_compile_unit};
use crate::ir::{
    AggregateArgument, AggregateArgumentSource, AggregateLocation, BoolComparisonOperator,
    BoolLocation, BoolLogicalOperator, BoolValue, BorrowArgument, BorrowSource, CallTarget,
    DirectAggregateArgument, FallibleFailureMode, Function, I32ComparisonOperator, I32Location,
    I32Value, Instruction, IrModule, ScalarArgument, SliceElementAddressKind, SliceElementIndex,
    SliceLocation, SliceValue, StrLocation, StrValue, Type, U8Location, U8Value, UsizeLocation,
    UsizeValue,
};
use crate::source::SourceMap;
use crate::target::DEFAULT_TARGET;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

mod aggregates;
mod arrays;
mod basic;
mod call_evaluation;
mod calls;
mod control_flow;
mod diagnostics;
mod drops;
mod generics;
mod imports;
mod optional_fallible;
mod payload_enums;
mod scalars;
mod slices_strings_pointers;

fn lower_text(text: &str) -> IrModule {
    let diagnostics = lower_text_diagnostics(text);
    match diagnostics.as_slice() {
        [] => {
            let fixture = analyze_text_fixture(text);
            lower_executable(&fixture.analysis, &fixture.sources).unwrap()
        }
        diagnostics => panic!("unexpected diagnostics: {diagnostics:?}"),
    }
}

fn lower_text_with_std_error(text: &str) -> IrModule {
    lower_text_with_nocter_home_files(text, &[std_error_file()])
}

fn lower_text_with_nocter_home_files(text: &str, home_files: &[(&str, &str)]) -> IrModule {
    let fixture = analyze_text_fixture_with_nocter_home_files(text, home_files);
    lower_executable(&fixture.analysis, &fixture.sources).unwrap()
}

fn lower_named_function(text: &str, function_name: &str) -> Function {
    lower_named_function_with_signatures(
        text,
        function_name,
        context::FunctionSignatures::new(HashMap::new()),
    )
    .unwrap()
}

fn lower_named_function_with_signatures(
    text: &str,
    function_name: &str,
    function_signatures: context::FunctionSignatures,
) -> Result<Function, Vec<Diagnostic>> {
    let fixture = analyze_text_fixture(text);
    let analysis = &fixture.analysis;
    let root = analysis.root_file().unwrap();
    let Some(crate::ast::Item::Function(function)) = root.ast.items.iter().find(|item| {
        matches!(item, crate::ast::Item::Function(function) if function.name == function_name)
    }) else {
        panic!("missing function `{function_name}`");
    };
    let resolved_sources = analysis
        .files
        .iter()
        .map(|file| (file.ast.span.source, &file.resolved))
        .collect();

    functions::lower_function(
        function,
        &HashMap::new(),
        function_name.to_string(),
        &fixture.sources,
        CallTarget::same_file(function_name),
        function_signatures,
        context::FunctionNames::default(),
        root.ast.span.source,
        &root.resolved,
        &root.typecheck_facts,
        resolved_sources,
        context::ErrorPayloads::default(),
    )
}

fn lower_named_function_with_nocter_home_files(
    text: &str,
    function_name: &str,
    home_files: &[(&str, &str)],
) -> Function {
    let fixture = analyze_text_fixture_with_nocter_home_files(text, home_files);
    let analysis = &fixture.analysis;
    let root = analysis.root_file().unwrap();
    let target = CallTarget::same_file(function_name);
    let index = FunctionIndex::new(analysis, root.ast.span.source);
    let function = index.definition(&target).unwrap();

    function
        .lower(
            target,
            &fixture.sources,
            index.signatures(),
            index.names(),
            index.error_payloads(root.ast.span.source),
            index.resolved_sources(),
            root.ast.span.source,
        )
        .unwrap()
}

fn lower_imported_named_function_with_nocter_home_files(
    text: &str,
    function_name: &str,
    home_files: &[(&str, &str)],
) -> Function {
    let fixture = analyze_text_fixture_with_nocter_home_files(text, home_files);
    let analysis = &fixture.analysis;
    let root = analysis.root_file().unwrap();
    let imported_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == function_name)
                })
        })
        .map(|file| file.ast.span.source)
        .unwrap();
    let target = CallTarget::imported(imported_source, function_name);
    let index = FunctionIndex::new(analysis, root.ast.span.source);
    let function = index.definition(&target).unwrap();

    function
        .lower(
            target,
            &fixture.sources,
            index.signatures(),
            index.names(),
            index.error_payloads(root.ast.span.source),
            index.resolved_sources(),
            root.ast.span.source,
        )
        .unwrap()
}

fn lower_named_function_diagnostics_with_signatures(
    text: &str,
    function_name: &str,
    function_signatures: context::FunctionSignatures,
) -> Vec<Diagnostic> {
    match lower_named_function_with_signatures(text, function_name, function_signatures) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

fn set_return_i32(value: i32) -> Instruction {
    Instruction::SetI32 {
        destination: I32Location::Return,
        value: i32_const(value),
    }
}

fn set_return_usize(value: u64) -> Instruction {
    Instruction::SetUsize {
        destination: UsizeLocation::Return,
        value: usize_const(value),
    }
}

fn tail_call(function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::TailCall {
        target: CallTarget::same_file(function),
        arguments: i32_arguments(arguments),
    }
}

fn call_i32(destination: I32Location, function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::CallI32 {
        destination,
        target: CallTarget::same_file(function),
        arguments: i32_arguments(arguments),
    }
}

fn call_usize(
    destination: UsizeLocation,
    function: &str,
    arguments: Vec<ScalarArgument>,
) -> Instruction {
    Instruction::CallUsize {
        destination,
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn call_u8(destination: U8Location, function: &str, arguments: Vec<ScalarArgument>) -> Instruction {
    Instruction::CallU8 {
        destination,
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn call_bool(
    destination: BoolLocation,
    function: &str,
    arguments: Vec<ScalarArgument>,
) -> Instruction {
    Instruction::CallBool {
        destination,
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn call_void(function: &str, arguments: Vec<ScalarArgument>) -> Instruction {
    Instruction::CallVoid {
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn call_target_name_is(target: &CallTarget, expected: &str) -> bool {
    match target {
        CallTarget::SameFile(name) | CallTarget::Imported { name, .. } => name == expected,
    }
}

fn call_str(
    destination: StrLocation,
    function: &str,
    arguments: Vec<ScalarArgument>,
) -> Instruction {
    Instruction::CallStr {
        destination,
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn call_slice(
    destination: SliceLocation,
    function: &str,
    arguments: Vec<ScalarArgument>,
) -> Instruction {
    Instruction::CallSlice {
        destination,
        target: CallTarget::same_file(function),
        arguments,
    }
}

fn function_signatures(signatures: Vec<(&str, Type, Vec<Type>)>) -> context::FunctionSignatures {
    context::FunctionSignatures::from_call_targets(
        signatures
            .into_iter()
            .map(|(name, return_type, parameter_types)| {
                (
                    CallTarget::same_file(name),
                    context::FunctionSignature {
                        return_type,
                        parameter_types: Some(parameter_types),
                        parameter_abi_word_count: None,
                        success_return_passing: None,
                    },
                )
            })
            .collect(),
    )
}

fn assert_contains_fallible_direct_aggregate_catch_call(
    function: &Function,
    expected_destination: AggregateLocation,
    expected_target: &str,
) {
    let Some(Instruction::CallFallibleDirectAggregate {
        destination,
        target,
        arguments,
        layout,
        failure_mode:
            FallibleFailureMode::Catch {
                code,
                message,
                instructions,
            },
    }) = function.instructions.iter().find(|instruction| {
        matches!(
            instruction,
            Instruction::CallFallibleDirectAggregate {
                failure_mode: FallibleFailureMode::Catch { .. },
                ..
            }
        )
    })
    else {
        panic!("missing fallible direct aggregate catch call: {function:?}");
    };

    assert_eq!(*destination, expected_destination);
    assert_eq!(target, &CallTarget::same_file(expected_target));
    assert_eq!(arguments, &Vec::<ScalarArgument>::new());
    assert_eq!(*layout, ValueLayout::new(16, 8));
    assert_eq!(*code, StrLocation::Local(0));
    assert_eq!(*message, StrLocation::Local(2));
    assert_eq!(
        instructions,
        &vec![Instruction::ReturnFallibleFailure {
            code: StrValue::StaticBytes(b"app.source".to_vec()),
            message: StrValue::Location(StrLocation::Local(2)),
        }]
    );
}

fn readonly_u8_slice_type() -> Type {
    Type::Slice {
        is_readwrite: false,
    }
}

fn readwrite_u8_slice_type() -> Type {
    Type::Slice { is_readwrite: true }
}

fn i32_arguments(arguments: Vec<I32Value>) -> Vec<ScalarArgument> {
    arguments.into_iter().map(ScalarArgument::I32).collect()
}

fn i32_const(value: i32) -> I32Value {
    I32Value::Const(value)
}

fn i32_param(index: usize) -> I32Value {
    I32Value::Location(I32Location::Parameter(index))
}

fn i32_local(index: usize) -> I32Value {
    I32Value::Location(I32Location::Local(index))
}

fn u8_const(value: u8) -> U8Value {
    U8Value::Const(value)
}

fn u8_param(index: usize) -> U8Value {
    U8Value::Location(U8Location::Parameter(index))
}

fn u8_local(index: usize) -> U8Value {
    U8Value::Location(U8Location::Local(index))
}

fn usize_const(value: u64) -> UsizeValue {
    UsizeValue::Const(value)
}

fn usize_slice_len(location: SliceLocation) -> UsizeValue {
    UsizeValue::SliceLen(location)
}

fn usize_slice_index(location: SliceLocation, index: UsizeValue) -> UsizeValue {
    UsizeValue::SliceIndex {
        source: location,
        index: Box::new(index),
    }
}

fn usize_param(index: usize) -> UsizeValue {
    UsizeValue::Location(UsizeLocation::Parameter(index))
}

fn str_static(bytes: &[u8]) -> ScalarArgument {
    ScalarArgument::Str(str_static_value(bytes))
}

fn str_static_value(bytes: &[u8]) -> StrValue {
    StrValue::StaticBytes(bytes.to_vec())
}

fn usize_local(index: usize) -> UsizeValue {
    UsizeValue::Location(UsizeLocation::Local(index))
}

fn bool_param(index: usize) -> BoolValue {
    BoolValue::Location(BoolLocation::Parameter(index))
}

fn lower_text_diagnostics(text: &str) -> Vec<Diagnostic> {
    let fixture = analyze_text_fixture(text);
    match lower_executable(&fixture.analysis, &fixture.sources) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics,
    }
}

struct LoweringFixture {
    sources: SourceMap,
    analysis: CompileUnitAnalysis,
}

fn analyze_text(text: &str) -> CompileUnitAnalysis {
    analyze_text_with_nocter_home_files(text, &[])
}

fn analyze_text_with_nocter_home_files(
    text: &str,
    home_files: &[(&str, &str)],
) -> CompileUnitAnalysis {
    analyze_text_fixture_with_nocter_home_files(text, home_files).analysis
}

fn analyze_text_fixture(text: &str) -> LoweringFixture {
    analyze_text_fixture_with_nocter_home_files(text, &[])
}

fn analyze_text_fixture_with_nocter_home_files(
    text: &str,
    home_files: &[(&str, &str)],
) -> LoweringFixture {
    let mut sources = SourceMap::new();
    let source = sources.add_source("app.nct", None, text);
    let temp_root = make_temp_project();
    let nocter_home = make_nocter_home(&temp_root);
    write_nocter_home_files(&nocter_home, home_files);
    let unit: CompileUnit = load_compile_unit(
        &mut sources,
        source,
        &FrontendOptions {
            nocter_home: Some(nocter_home),
            source_root: None,
            target: DEFAULT_TARGET.to_string(),
        },
    )
    .unwrap();
    let analysis = analyze_executable_compile_unit(&sources, &unit);
    let diagnostics = analysis.diagnostics();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    LoweringFixture { sources, analysis }
}

fn std_error_file() -> (&'static str, &'static str) {
    (
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error {
    return new_error(code, message)
}
"#,
    )
}

fn std_io_file() -> (&'static str, &'static str) {
    (
        "std/io.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!

#target("arm64-darwin")
pub(nocter) primitive write_bytes_raw(fd: i32, bytes: &[u8]): void!

#target("arm64-darwin")
pub(nocter) primitive read_bytes_raw(fd: i32, buffer: &+[u8]): usize!

#target("arm64-darwin")
pub(nocter) primitive close_fd_raw(fd: i32): void

pub func print(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    )
}

fn std_string_bytes_file() -> (&'static str, &'static str) {
    (
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    )
}

fn std_process_file() -> (&'static str, &'static str) {
    (
        "std/process.nct",
        r#"use std/os.trap

#target("arm64-darwin")
pub(nocter) primitive exit_raw(code: i32): never

pub func exit(code: i32): never {
    exit_raw(code)
}

pub func abort(): never {
    trap()
}
"#,
    )
}

fn std_os_file() -> (&'static str, &'static str) {
    (
        "std/os.nct",
        r#"#target("arm64-darwin")
pub(nocter) primitive trap(): never

#target("arm64-darwin")
pub(nocter) primitive unreachable(): never
"#,
    )
}

fn write_nocter_home_files(home: &Path, files: &[(&str, &str)]) {
    for (relative, text) in files {
        let path = home.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }
}

fn make_temp_project() -> PathBuf {
    let unique = format!(
        "nocter-ir-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEMP_PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed),
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).unwrap();
    root
}

fn make_nocter_home(root: &Path) -> PathBuf {
    let home = root.join(".nocter");
    fs::create_dir_all(home.join("std")).unwrap();
    fs::write(home.join("std/prelude.nct"), "").unwrap();
    home
}
