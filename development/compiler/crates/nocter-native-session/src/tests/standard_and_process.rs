use super::*;

#[test]
fn bundled_standard_library_crosses_the_complete_target_session() {
    let root = nocter_test_support::standard_library_root();
    let package = PackageIdentity::new("toolchain:std");
    let resolved = resolved_standard(&root, &package);
    let roots = module_roots(&root)
        .into_iter()
        .map(|path| ModuleIdentity::new(package.clone(), path))
        .collect();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![resolved]),
        roots,
        bundled_standard_toolchain(&package),
    ))
    .unwrap();
    let diagnostics = unit.syntax_diagnostics();
    let source_names = unit
        .sources()
        .iter()
        .map(|source| (source.id(), source.name().as_str()))
        .collect::<Vec<_>>();
    assert!(
        diagnostics.is_empty(),
        "bundled standard library has syntax diagnostics: {diagnostics:#?}\nsources: {source_names:#?}"
    );
    let compiled = compile_for_test(unit);

    assert_eq!(
        compiled.program().toolchain().primitives().bindings().len(),
        PrimitiveRole::ALL.len()
    );
    assert!(
        compiled
            .program()
            .toolchain()
            .runtime_storage()
            .declaration(RuntimeStorageRole::NetworkOwner)
            .is_some()
    );
    assert_eq!(
        compiled.program().checked().bodies().len(),
        compiled
            .program()
            .checked()
            .graph()
            .declarations()
            .bodies()
            .len()
    );
}

#[test]
fn standard_unicode_lookup_contract_crosses_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"unicode\", module: \"./internal/unicode\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["internal", "unicode"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 1);
    let NativeTestTargetOutcome::Compiled(cases) = compiled.targets()[0].outcome() else {
        panic!("standard Unicode tests failed native compilation")
    };
    assert_eq!(cases.len(), 2);
    let output = TempPackage::new();
    for case in cases {
        execute_native_test(case.image(), &output.0, case.identity().name());
    }
}

#[test]
fn standard_subprocess_contract_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let helper = package_root.0.join("subprocess-helper");
    let missing = package_root.0.join("missing-executable");
    package_root.source(
        "main.nct",
        &format!(
            r#"use std/process.{{Command, ExitStatus}}
use std/string.String

noalloc func has_signal(status: ExitStatus): bool {{
    let _ = status.signal() otherwise {{ return false }}
    return true
}}

async func main(): i32 {{
    var path = String.copy("{}")
    var first = String.copy("alpha beta")
    var command = Command.new(&path as &str) catch _ {{ return 1 }}
    command.arg(&first as &str) catch _ {{ return 2 }}
    command.arg("") catch _ {{ return 3 }}
    path.clear()
    first.clear()

    let status = await command.status() catch _ {{ return 4 }}
    let code = status.code() otherwise {{ return 5 }}
    if status.success() || code != 7 || has_signal(status) {{ return 6 }}

    let missing = Command.new("{}") catch _ {{ return 8 }}
    let _status = await missing.status() catch failure {{
        if failure.has_code("std.process.not_found") {{ return 0 }}
        return 9
    }}
    return 10
}}
"#,
            helper.display(),
            missing.display(),
        ),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_subprocess_contract(image.image(), &package_root.0);
}

#[test]
#[allow(clippy::too_many_lines)] // Keeps one complete async-output source scenario contiguous.
fn standard_subprocess_output_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let helper = package_root.0.join("capture-helper");
    let empty = package_root.0.join("empty-capture-helper");
    let text = package_root.0.join("text-capture-helper");
    let signaled = package_root.0.join("signal-capture-helper");
    let early = package_root.0.join("early-close-capture-helper");
    let missing = package_root.0.join("missing-capture-helper");
    package_root.source(
        "main.nct",
        &format!(
            r#"use std/process.Command
use std/string.String

noalloc func matches_stream(
    bytes: &[u8],
    repeated: u8,
    repeated_len: usize,
    first_tail: u8,
    second_tail: u8,
): bool {{
    if bytes.len() != repeated_len + 2 {{ return false }}
    var index: usize = 0
    while index < repeated_len {{
        if bytes[index] != repeated {{ return false }}
        index += 1
    }}
    return bytes[repeated_len] == first_tail
        && bytes[repeated_len + 1] == second_tail
}}

async func main(): i32 {{
    let command = Command.new("{}") catch _ {{ return 1 }}
    let output = await command.output() catch _ {{ return 2 }}
    let code = output.status.code() otherwise {{ return 3 }}
    if output.status.success() || code != 23 {{ return 4 }}

    let stdout: &[u8] = &output.stdout as &[u8]
    let stderr: &[u8] = &output.stderr as &[u8]
    if !matches_stream(stdout, 79, 262144, 0, 255) {{ return 5 }}
    if !matches_stream(stderr, 69, 262144, 0, 254) {{ return 6 }}

    let empty = Command.new("{}") catch _ {{ return 7 }}
    let empty_output = await empty.output() catch _ {{ return 8 }}
    if !empty_output.status.success() || empty_output.stdout.len() != 0
        || empty_output.stderr.len() != 0 {{ return 9 }}

    let text = Command.new("{}") catch _ {{ return 10 }}
    let text_output = await text.output() catch _ {{ return 11 }}
    if !text_output.status.success() || text_output.stdout.len() != 6
        || text_output.stdout[0] != 104 || text_output.stdout[1] != 101
        || text_output.stdout[2] != 108 || text_output.stdout[3] != 108
        || text_output.stdout[4] != 111 || text_output.stdout[5] != 10
        || text_output.stderr.len() != 8 || text_output.stderr[0] != 119
        || text_output.stderr[1] != 97 || text_output.stderr[2] != 114
        || text_output.stderr[3] != 110 || text_output.stderr[4] != 105
        || text_output.stderr[5] != 110 || text_output.stderr[6] != 103
        || text_output.stderr[7] != 10 {{ return 12 }}

    let signaled = Command.new("{}") catch _ {{ return 13 }}
    let signal_output = await signaled.output() catch _ {{ return 14 }}
    let signal = signal_output.status.signal() otherwise {{ return 15 }}
    if signal != 15 || signal_output.stdout.len() != 10
        || signal_output.stdout[0] != 115 || signal_output.stdout[1] != 105
        || signal_output.stdout[2] != 103 || signal_output.stdout[3] != 110
        || signal_output.stdout[4] != 97 || signal_output.stdout[5] != 108
        || signal_output.stdout[6] != 45 || signal_output.stdout[7] != 111
        || signal_output.stdout[8] != 117 || signal_output.stdout[9] != 116
        || signal_output.stderr.len() != 12 || signal_output.stderr[0] != 115
        || signal_output.stderr[1] != 105 || signal_output.stderr[2] != 103
        || signal_output.stderr[3] != 110 || signal_output.stderr[4] != 97
        || signal_output.stderr[5] != 108 || signal_output.stderr[6] != 45
        || signal_output.stderr[7] != 101 || signal_output.stderr[8] != 114
        || signal_output.stderr[9] != 114 || signal_output.stderr[10] != 111
        || signal_output.stderr[11] != 114 {{ return 16 }}

    var early_input = String.with_capacity(1048576)
    var early_block: usize = 0
    while early_block < 16384 {{
        early_input.push_str("IIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIIII")
        early_block += 1
    }}
    var early = Command.new("{}") catch _ {{ return 17 }}
    early.input(early_input.bytes())
    let early_output = await early.output() catch _ {{ return 18 }}
    if !early_output.status.success() || early_output.stdout.len() != 0
        || early_output.stderr.len() != 0 {{ return 19 }}

    var attempt: usize = 0
    while attempt < 48 {{
        let repeated = Command.new("{}") catch _ {{ return 20 }}
        let repeated_output = await repeated.output() catch _ {{ return 21 }}
        if !repeated_output.status.success() || repeated_output.stdout.len() != 0
            || repeated_output.stderr.len() != 0 {{ return 22 }}
        attempt += 1
    }}

    let missing = Command.new("{}") catch _ {{ return 23 }}
    let _missing_output = await missing.output() catch failure {{
        if failure.has_code("std.process.not_found") {{ return 0 }}
        return 24
    }}
    return 25
}}
"#,
            helper.display(),
            empty.display(),
            text.display(),
            signaled.display(),
            early.display(),
            empty.display(),
            missing.display(),
        ),
    );
    compile_and_execute_subprocess_output(&package_root.0, &standard_root);
}

#[test]
fn standard_spawned_child_and_pipe_values_cross_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let helper = package_root.0.join("streaming-child-helper");
    package_root.source(
        "main.nct",
        &format!(
            r#"use std/io.{{BlockingReader, BlockingWriter}}
use std/io
use std/process.{{Command, ProcessIo}}
use std/vec.Vec

struct ByteSink {{ bytes: Vec<u8> }}

construct ByteSink {{
    func empty(): Self {{ return ByteSink {{ bytes: Vec.empty() }} }}
}}

instance ByteSink {{
    impl BlockingWriter

    blocking method &+self.write_blocking(value: &[u8]): void! {{
        var index: usize = 0
        while index < value.len() {{
            self.bytes.push(value[index])
            index += 1
        }}
        return
    }}
}}

func is_none<T>(value: T?): bool {{
    let _ = move value otherwise {{ return true }}
    return false
}}

blocking func main(): i32 {{
    let command = Command.new("{}") catch _ {{ return 1 }}
    var child = command.spawn_blocking(ProcessIo.piped()) catch _ {{ return 2 }}

    var stdin = child.take_stdin() otherwise {{ return 3 }}
    if !is_none(child.take_stdin()) {{ return 4 }}
    stdin.write_text_blocking("request\n") catch _ {{ return 5 }}
    stdin.close()

    var stdout = child.take_stdout() otherwise {{ return 6 }}
    if !is_none(child.take_stdout()) {{ return 7 }}
    var output = ByteSink.empty()
    let copied = io.copy_blocking(&+stdout, &+output) catch _ {{ return 8 }}

    var stderr = child.take_stderr() otherwise {{ return 9 }}
    if !is_none(child.take_stderr()) {{ return 10 }}
    let diagnostic = stderr.read_to_end_blocking() catch _ {{ return 11 }}

    let status = child.wait_blocking() catch _ {{ return 12 }}
    if !status.success() {{ return 13 }}
    if copied != 9 || output.bytes.len() != 9 || output.bytes[0] != 114
        || output.bytes[7] != 101 || output.bytes[8] != 10 {{
        return 14
    }}
    if diagnostic.len() != 5 || diagnostic[0] != 110 || diagnostic[4] != 10 {{
        return 15
    }}
    return 0
}}
"#,
            helper.display(),
        ),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_spawned_child_contract(image.image(), &package_root.0);
}

#[test]
fn standard_async_spawn_and_pipe_values_cross_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let helper = package_root.0.join("streaming-child-helper");
    package_root.source(
        "main.nct",
        &format!(
            r#"use std/io.{{Reader, Writer}}
use std/io
use std/process.{{Command, ProcessIo, Stdio}}
use std/vec.Vec

struct ByteSink {{ bytes: Vec<u8> }}

construct ByteSink {{
    func empty(): Self {{ return ByteSink {{ bytes: Vec.empty() }} }}
}}

instance ByteSink {{
    impl Writer

    async method &+self.write(value: &[u8]): void! {{
        var index: usize = 0
        while index < value.len() {{
            self.bytes.push(value[index])
            index += 1
        }}
        return
    }}
}}

func is_none<T>(value: T?): bool {{
    let _ = move value otherwise {{ return true }}
    return false
}}

async func main(): i32 {{
    let command = Command.new("{}") catch _ {{ return 1 }}
    var process_io = ProcessIo.piped()
    process_io.stderr(Stdio.null)
    var child = await command.spawn(move process_io) catch _ {{ return 2 }}

    var stdin = child.take_stdin() otherwise {{ return 3 }}
    await stdin.write_text("request\n") catch _ {{ return 4 }}
    stdin.close()

    var stdout = child.take_stdout() otherwise {{ return 5 }}
    var output = ByteSink.empty()
    let copied = await io.copy(&+stdout, &+output) catch _ {{ return 6 }}
    if !is_none(child.take_stderr()) {{ return 7 }}

    let status = await child.wait() catch _ {{ return 8 }}
    if !status.success() {{ return 9 }}
    if copied != 9 || output.bytes.len() != 9 || output.bytes[0] != 114
        || output.bytes[7] != 101 || output.bytes[8] != 10 {{
        return 10
    }}

    let missing = Command.new("{}") catch _ {{ return 11 }}
    let _missing_child = await missing.spawn(ProcessIo.inherit()) catch failure {{
        if failure.has_code("std.process.not_found") {{ return 0 }}
        return 12
    }}
    return 13
}}
"#,
            helper.display(),
            package_root.0.join("missing-executable").display(),
        ),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_spawned_child_contract(image.image(), &package_root.0);
}

#[test]
fn standard_files_and_process_pipes_compose_through_one_async_copy_contract() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let helper = package_root.0.join("streaming-child-helper");
    package_root.source(
        "main.nct",
        &format!(
            r#"use std/fs
use std/io.File
use std/io
use std/process.{{ChildStdin, ChildStdout, Command, ProcessIo, Stdio}}
use std/task

async func send_file(source_path: &str, destination: ChildStdin): usize! {{
    var source = await File.open(source_path)?
    var output = move destination
    let copied = await io.copy(&+source, &+output) catch failure {{
        await source.close() catch _ {{}}
        output.close()
        return move failure
    }}
    await source.close()?
    output.close()
    return copied
}}

async func receive_file(source: ChildStdout, destination_path: &str): usize! {{
    var input = move source
    var destination = await File.create(destination_path)?
    let copied = await io.copy(&+input, &+destination) catch failure {{
        input.close()
        await destination.close() catch _ {{}}
        return move failure
    }}
    input.close()
    await destination.close()?
    return copied
}}

async func main(): i32! {{
    await fs.write_text("pipeline-input", "request\n")?
    let command = Command.new("{}")?
    var process_io = ProcessIo.piped()
    process_io.stderr(Stdio.null)
    var child = await command.spawn(move process_io)?
    let input = child.take_stdin() otherwise {{ return 1 }}
    let output = child.take_stdout() otherwise {{ return 2 }}
    let transfers = await task.join(
        send_file("pipeline-input", move input),
        receive_file(move output, "pipeline-output"),
    )
    let sent = move transfers.0?
    let received = move transfers.1?
    let status = await child.wait()?
    if !status.success() || sent != 8 || received != 9 {{ return 3 }}
    let text = await fs.read_to_string("pipeline-output")?
    if (&text as &str) != "response\n" {{ return 4 }}
    return 0
}}
"#,
            helper.display(),
        ),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_spawned_child_contract(image.image(), &package_root.0);
}

const CONFIGURED_SUBPROCESS_HELPERS_SOURCE: &str =
    include_str!("../../../../tests/fixtures/native/session/configured-subprocess-helpers.nct");

#[test]
fn configured_subprocess_crosses_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let workspace = package_root.0.join("configured-workspace");
    let inherited_helper = package_root.0.join("inherited-helper");
    let transfer_helper = package_root.0.join("transfer-helper");
    let empty_helper = package_root.0.join("empty-input-helper");
    let early_close_helper = package_root.0.join("early-close-helper");
    let missing_directory = package_root.0.join("missing-directory");
    package_root.source(
        "main.nct",
        &format!(
            r#"{CONFIGURED_SUBPROCESS_HELPERS_SOURCE}
blocking func main(): i32 {{
    var exact = Command.new("./environment-helper") catch _ {{ return 1 }}
    exact.current_dir("{}") catch _ {{ return 2 }}
    exact.clear_env()
    exact.env("KEEP", "first") catch _ {{ return 3 }}
    exact.env("KEEP", "final=value") catch _ {{ return 4 }}
    exact.env("REMOVE", "present") catch _ {{ return 5 }}
    exact.remove_env("REMOVE") catch _ {{ return 6 }}
    let exact_status = exact.status_blocking() catch _ {{ return 7 }}
    if !exact_status.success() {{
        return exact_status.code() otherwise {{ return 8 }}
    }}

    var inherited = Command.new("{}") catch _ {{ return 9 }}
    inherited.env("NOCTER_CHANGED", "child=value") catch _ {{ return 10 }}
    inherited.remove_env("NOCTER_REMOVED") catch _ {{ return 11 }}
    let inherited_status = inherited.status_blocking() catch _ {{ return 12 }}
    if !inherited_status.success() {{
        return inherited_status.code() otherwise {{ return 13 }}
    }}

    let input_byte: u8 = 73
    let stdout_byte: u8 = 79
    let stderr_byte: u8 = 69
    let transfer_count: usize = 131072
    let input = repeated(input_byte, transfer_count)
    var transfer = Command.new("{}") catch _ {{ return 14 }}
    transfer.input(&input)
    let output = transfer.output_blocking() catch _ {{ return 15 }}
    if !output.status.success() || output.stdout.len() != transfer_count * 2
        || output.stderr.len() != transfer_count {{ return 16 }}
    if !range_matches(&output.stdout, 0, transfer_count, stdout_byte)
        || !range_matches(&output.stdout, transfer_count, transfer_count, input_byte)
        || !range_matches(&output.stderr, 0, transfer_count, stderr_byte) {{ return 17 }}

    let empty: Vec<u8> = Vec.empty()
    var empty_command = Command.new("{}") catch _ {{ return 18 }}
    empty_command.input(&empty)
    let empty_status = empty_command.status_blocking() catch _ {{ return 19 }}
    if !empty_status.success() {{ return 20 }}

    let early_bytes = repeated(input_byte, 1048576)
    var early = Command.new("{}") catch _ {{ return 21 }}
    early.input(&early_bytes)
    let early_output = early.output_blocking() catch _ {{ return 22 }}
    if !early_output.status.success() || early_output.stdout.len() != 0
        || early_output.stderr.len() != 0 {{ return 23 }}

    var bad_directory = Command.new("./never-executed") catch _ {{ return 24 }}
    bad_directory.current_dir("{}") catch _ {{ return 25 }}
    if !status_fails_with(move bad_directory, "std.process.current_directory_failed") {{ return 26 }}

    var bad_output_directory = Command.new("./never-executed") catch _ {{ return 27 }}
    bad_output_directory.current_dir("{}") catch _ {{ return 28 }}
    if !output_fails_with(move bad_output_directory, "std.process.current_directory_failed") {{
        return 29
    }}
    return 0
}}
"#,
            workspace.display(),
            inherited_helper.display(),
            transfer_helper.display(),
            empty_helper.display(),
            early_close_helper.display(),
            missing_directory.display(),
            missing_directory.display(),
        ),
    );
    compile_and_execute_configured_subprocess(&package_root.0, &standard_root);
}

#[test]
fn standard_process_internal_contracts_cross_native_tests() {
    let standard_root = nocter_test_support::standard_library_root();
    let standard_package = PackageIdentity::new("toolchain:std");
    let mut root_source = fs::read_to_string(standard_root.join("index.nct")).unwrap();
    root_source.push_str("\n#test: { name: \"process\", module: \"./process\" }\n");
    root_source.push_str("#test: { name: \"darwin-pair\", module: \"./internal/os/darwin\" }\n");
    let mut overlay = SourceOverlay::builder();
    overlay
        .insert_source(
            standard_root.join("index.nct"),
            SourceOverride::new(root_source.into_bytes()),
        )
        .unwrap();
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph_with_overlay(
            vec![resolved_standard(&standard_root, &standard_package)],
            overlay.finish(),
        ),
        vec![
            ModuleIdentity::new(standard_package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(standard_package.clone(), ["process"]),
            ModuleIdentity::new(standard_package.clone(), ["internal", "os", "darwin"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(compiled.targets().len(), 2);
    let output = TempPackage::new();
    let mut case_count = 0;
    for target in compiled.targets() {
        let NativeTestTargetOutcome::Compiled(cases) = target.outcome() else {
            panic!("standard process tests failed native compilation")
        };
        for case in cases {
            case_count += 1;
            execute_native_test(case.image(), &output.0, case.identity().name());
        }
    }
    assert_eq!(case_count, 14);
}

#[test]
#[allow(clippy::too_many_lines)] // Keeps one complete child-lifecycle source scenario contiguous.
fn standard_subprocess_failures_and_lifecycle_cross_the_complete_native_session() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let success = package_root.0.join("success-helper");
    let nonzero = package_root.0.join("nonzero-helper");
    let exit_127 = package_root.0.join("exit-127-helper");
    let signaled = package_root.0.join("signal-helper");
    let missing = package_root.0.join("missing-helper");
    let denied = package_root.0.join("denied-helper");
    let invalid = package_root.0.join("invalid-helper");
    let arguments = package_root.0.join("argument-helper");
    package_root.source(
        "main.nct",
        &format!(
            r#"use std/process.{{Command, ExitStatus, ProcessIo}}
use std/string.String

noalloc func exited_with(status: ExitStatus, expected: i32): bool {{
    let code = status.code() otherwise {{ return false }}
    let _signal = status.signal() otherwise {{ return code == expected }}
    return false
}}

noalloc func signaled_with(status: ExitStatus, expected: i32): bool {{
    let signal = status.signal() otherwise {{ return false }}
    let _code = status.code() otherwise {{ return signal == expected }}
    return false
}}

blocking func fails_with(command: Command, code: &str): bool {{
    let _status = command.status_blocking() catch failure {{ return failure.has_code(code) }}
    return false
}}

func is_none<T>(value: T?): bool {{
    let _ = move value otherwise {{ return true }}
    return false
}}

blocking func main(): i32 {{
    let success = Command.new("{}") catch _ {{ return 1 }}
    let success_status = success.status_blocking() catch _ {{ return 2 }}
    if !success_status.success() || !exited_with(success_status, 0) {{ return 3 }}

    let nonzero = Command.new("{}") catch _ {{ return 4 }}
    let nonzero_status = nonzero.status_blocking() catch _ {{ return 5 }}
    if nonzero_status.success() || !exited_with(nonzero_status, 23) {{ return 6 }}

    let ordinary_127 = Command.new("{}") catch _ {{ return 7 }}
    let ordinary_127_status = ordinary_127.status_blocking() catch _ {{ return 8 }}
    if ordinary_127_status.success() || !exited_with(ordinary_127_status, 127) {{ return 9 }}

    let signaled = Command.new("{}") catch _ {{ return 10 }}
    let signal_status = signaled.status_blocking() catch _ {{ return 11 }}
    if signal_status.success() || !signaled_with(signal_status, 15) {{ return 12 }}

    let missing = Command.new("{}") catch _ {{ return 13 }}
    if !fails_with(move missing, "std.process.not_found") {{ return 14 }}

    let denied = Command.new("{}") catch _ {{ return 15 }}
    if !fails_with(move denied, "std.process.permission_denied") {{ return 16 }}

    let invalid = Command.new("{}") catch _ {{ return 17 }}
    if !fails_with(move invalid, "std.process.invalid_input") {{ return 18 }}

    let relative = Command.new("./relative-helper") catch _ {{ return 19 }}
    let relative_status = relative.status_blocking() catch _ {{ return 20 }}
    if !exited_with(relative_status, 31) {{ return 21 }}

    var argument_command = Command.new("{}") catch _ {{ return 22 }}
    argument_command.arg("") catch _ {{ return 23 }}
    argument_command.arg("alpha beta") catch _ {{ return 24 }}
    var rejected_nul = false
    argument_command.arg("bad\0argument") catch failure {{
        if !failure.has_code("std.process.invalid_input") {{ return 25 }}
        rejected_nul = true
    }}
    if !rejected_nul {{ return 26 }}
    let argument_status = argument_command.status_blocking() catch _ {{ return 27 }}
    if !exited_with(argument_status, 0) {{ return 28 }}

    var oversized = String.with_capacity(2097152)
    var block: usize = 0
    while block < 32768 {{
        oversized.push_str("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
        block += 1
    }}
    var oversized_command = Command.new("{}") catch _ {{ return 29 }}
    oversized_command.arg(&oversized as &str) catch _ {{ return 30 }}
    if !fails_with(move oversized_command, "std.process.invalid_input") {{ return 31 }}

    var attempt: usize = 0
    while attempt < 96 {{
        let repeated = Command.new("{}") catch _ {{ return 32 }}
        if !fails_with(move repeated, "std.process.not_found") {{ return 33 }}
        attempt += 1
    }}

    let final_success = Command.new("{}") catch _ {{ return 34 }}
    let final_status = final_success.status_blocking() catch _ {{ return 35 }}
    if !exited_with(final_status, 0) {{ return 36 }}

    let terminate_command = Command.new("{}") catch _ {{ return 37 }}
    var terminated = terminate_command.spawn_blocking(ProcessIo.inherit()) catch _ {{ return 38 }}
    let initially_ready = terminated.try_wait() catch _ {{ return 39 }}
    if !is_none(initially_ready) {{ return 40 }}
    terminated.terminate() catch _ {{ return 41 }}
    var terminate_attempts: usize = 0
    var terminate_observed = false
    while terminate_attempts < 1000000 {{
        let candidate = terminated.try_wait() catch _ {{ return 42 }}
        let observed = candidate otherwise {{
            terminate_attempts += 1
            continue
        }}
        if !signaled_with(observed, 15) {{ return 43 }}
        let repeated = terminated.try_wait() catch _ {{ return 44 }}
        let cached = repeated otherwise {{ return 45 }}
        if !signaled_with(cached, 15) {{ return 46 }}
        let waited = terminated.wait_blocking() catch _ {{ return 47 }}
        if !signaled_with(waited, 15) {{ return 48 }}
        terminate_observed = true
        break
    }}
    if !terminate_observed {{ return 49 }}

    let kill_command = Command.new("{}") catch _ {{ return 50 }}
    var killed = kill_command.spawn_blocking(ProcessIo.inherit()) catch _ {{ return 51 }}
    killed.kill() catch _ {{ return 52 }}
    let killed_status = killed.wait_blocking() catch _ {{ return 53 }}
    if !signaled_with(killed_status, 9) {{ return 54 }}
    return 0
}}
"#,
            success.display(),
            nonzero.display(),
            exit_127.display(),
            signaled.display(),
            missing.display(),
            denied.display(),
            invalid.display(),
            arguments.display(),
            success.display(),
            missing.display(),
            success.display(),
            package_root.0.join("terminate-helper").display(),
            package_root.0.join("kill-helper").display(),
        ),
    );
    compile_and_execute_subprocess_lifecycle(&package_root.0, &standard_root);
}

#[test]
fn subprocess_timeout_cancellation_reaps_the_exact_child() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    let helper = package_root.0.join("timeout-helper");
    let pid_file = package_root.0.join("timeout-child.pid");
    package_root.source(
        "main.nct",
        &format!(
            r#"use std/io.Reader
use std/process.{{Command, ProcessIo, Stdio}}
use std/task
use std/task.Timeout
use std/time.Duration
use std/vec.Vec

async func main(): i32 {{
    var command = Command.new("{}") catch _ {{ return 1 }}
    command.arg("{}") catch _ {{ return 2 }}
    var io = ProcessIo.inherit()
    io.stdout(Stdio.pipe)
    var child = await command.spawn(move io) catch _ {{ return 3 }}
    var ready = child.take_stdout() otherwise {{ return 4 }}
    var marker: Vec<u8> = Vec [
        u8.truncate(0), u8.truncate(0), u8.truncate(0),
        u8.truncate(0), u8.truncate(0), u8.truncate(0),
    ]
    let received = await ready.read(&+marker) catch _ {{ return 5 }}
    if received == 0 {{ return 6 }}
    ready.close()
    let bounded = await task.with_timeout(
        child.wait(),
        Duration.from_milliseconds(250),
    )
    match move bounded {{
        Timeout.completed(_) {{ return 7 }}
        Timeout.elapsed {{ return 0 }}
    }}
}}
"#,
            helper.display(),
            pid_file.display(),
        ),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_subprocess_timeout_cancellation(image.image(), &package_root.0);
}

fn compile_and_execute_subprocess_lifecycle(package_root: &Path, standard_root: &Path) {
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.join("main.nct"),
        package_graph(vec![resolved_standard(standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_subprocess_lifecycle_contract(image.image(), package_root);
}

fn compile_and_execute_subprocess_output(package_root: &Path, standard_root: &Path) {
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.join("main.nct"),
        package_graph(vec![resolved_standard(standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_subprocess_output_contract(image.image(), package_root);
}

fn compile_and_execute_configured_subprocess(package_root: &Path, standard_root: &Path) {
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.join("main.nct"),
        package_graph(vec![resolved_standard(standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let compiled = compile_for_test(unit);
    let image = compile_native_image(ExecutableCompileRequest::only(compiled)).unwrap();
    execute_configured_subprocess_contract(image.image(), package_root);
}
