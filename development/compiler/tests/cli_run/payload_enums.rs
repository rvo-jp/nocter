use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_payloadless_enum_equality_exit_code() {
    let project = TempProject::new("cli-run-payloadless-enum-equality");
    let source = project.write_source(
        "payloadless_enum_equality.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let inferred = Choice.yes
    let annotated: Choice = Choice.yes
    if inferred == annotated && Choice.yes != Choice.no && choose() == Choice.maybe && stack_passed(1, 2, 3, 4, 5, 6, 7, 8, Choice.no) {
        return 42
    } else {
        return 1
    }
}

func choose(): Choice {
    return Choice.maybe
}

func stack_passed(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32, choice: Choice): bool {
    return choice == Choice.no
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_copy_payload_enum_construction_exit_code() {
    let project = TempProject::new("cli-run-copy-payload-enum-construction");
    let source = project.write_source(
        "copy_payload_enum_construction.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let local = Result.ok(10)
    let returned = make_ok()
    let direct_empty = Result.failed
    let empty = make_failed()
    return accept(move local) + accept(move returned) + accept(move direct_empty) + accept(move empty) + 38
}

func make_ok(): Result {
    return Result.ok(20)
}

func make_failed(): Result {
    return Result.failed
}

func accept(result: Result): i32 {
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_active_payload_enum_payload_at_scope_end_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-active-payload-scope-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_active_payload_scope_drop.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { code: 42 })
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_multi_field_payload_enum_payloads_in_reverse_order_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-multi-payload-scope-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_multi_payload_scope_drop.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { code: 10 }, Payload { code: 20 })
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(20),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_skips_inactive_payload_enum_payload_scope_drop_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-inactive-payload-scope-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_inactive_payload_scope_drop.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.failed
    return 42
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_replaced_payload_enum_payload_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-active-payload-replacement-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_active_payload_replacement_drop.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    var result = Result.ok(Payload { code: 42 })
    result = Result.failed
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_discarded_payload_enum_call_result_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-active-payload-discarded-call");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_active_payload_discarded_call.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    make()
    return 0
}

func make(): Result {
    return Result.ok(Payload { code: 42 })
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_payload_enum_call_result_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-active-payload-call-binding");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_active_payload_call_binding.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = make()
    return 0
}

func make(): Result {
    return Result.ok(Payload { code: 42 })
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_payload_enum_parameter_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-active-payload-parameter");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_active_payload_parameter.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    consume(Result.ok(Payload { code: 42 }))
    return 0
}

func consume(result: Result): void {
    return
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_generic_payload_enum_payload_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-generic-active-payload");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_generic_active_payload.nct",
        r#"use std/process.exit

struct Box<T> {
    code: i32
    value: T
}

impl<T> Box<T> {
    drop &+self {
        exit(self.code)
    }
}

enum Maybe<T> {
    some(value: T)
    absent
}

func main(): i32 {
    let result: Maybe<Box<i32>> = Maybe.some(Box<i32> { code: 42, value: 0 })
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_discard_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-discard");
    let source = project.write_source(
        "payload_enum_if_is_discard.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let ok = Result.ok(10)
    let failed = Result.failed
    let ok_score = score(move ok)
    let failed_score = score(move failed)
    return ok_score + failed_score
}

func score(result: Result): i32 {
    if result is Result.ok(_) {
        return 40
    } else {
        return 2
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_i32_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-i32-binding");
    let source = project.write_source(
        "payload_enum_if_is_i32_binding.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let ok = Result.ok(40)
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): i32 {
    if result is Result.ok(value) {
        return value
    }

    return 2
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_i32_binding_expression_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-i32-binding-expression");
    let source = project.write_source(
        "payload_enum_if_is_i32_binding_expression.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let ok = Result.ok(40)
    let failed = Result.failed
    return terminal(move ok) + value_score(move failed)
}

func terminal(result: Result): i32 {
    return if result is Result.ok(value) {
        value
    } else {
        1
    }
}

func value_score(result: Result): i32 {
    let scored = if result is Result.ok(value) {
        value
    } else {
        2
    }
    return scored
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_str_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-str-binding");
    let source = project.write_source(
        "payload_enum_if_is_str_binding.nct",
        r#"enum Message {
    text(value: &str)
    empty
}

func main(): i32 {
    let text = Message.text("Nocter")
    let empty = Message.empty
    return score(move text) + score(move empty)
}

func score(message: Message): i32 {
    if message is Message.text(text) {
        if text.len() == 6 {
            return 40
        }
        return 1
    }

    return 2
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_slice_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-slice-binding");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "payload_enum_if_is_slice_binding.nct",
        r#"use std/string.bytes

enum Result {
    ok(value: &[u8])
    failed
}

func main(): usize {
    let ok = Result.ok(bytes("Nocter"))
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): usize {
    if result is Result.ok(value) {
        return value.len()
    }

    return 36
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_expression_slice_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-expression-slice-binding");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "payload_enum_if_is_expression_slice_binding.nct",
        r#"use std/string.bytes

enum Result {
    ok(value: &[u8])
    failed
}

func main(): usize {
    let ok = Result.ok(bytes("Nocter"))
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): usize {
    let scored = if result is Result.ok(value) {
        value.len()
    } else {
        36
    }
    return scored
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_call_target_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-call-target-binding");
    let source = project.write_source(
        "payload_enum_if_is_call_target_binding.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    if make_ok() is Result.ok(value) {
        return value
    }

    return 0
}

func make_ok(): Result {
    return Result.ok(42)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_forced_call_target_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-forced-call-target");
    let source = project.write_source(
        "payload_enum_if_is_forced_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    if make_ok()! is Result.ok(value) {
        return value
    }

    return 0
}

func make_ok(): Result! {
    return Result.ok(42)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_transfers_owned_payload_from_forced_call_target_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-forced-owned-target");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_if_is_forced_owned_target.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    if make_ok()! is Result.ok(value) {
        return 1
    }

    return 0
}

func make_ok(): Result! {
    return Result.ok(Payload { code: 42 })
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_propagated_call_target_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-propagated-call-target");
    let source = project.write_source(
        "payload_enum_if_is_propagated_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32! {
    if make_ok()? is Result.ok(value) {
        return value
    }

    return 0
}

func make_ok(): Result! {
    return Result.ok(42)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_caught_call_target_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-caught-call-target");
    let source = project.write_source(
        "payload_enum_if_is_caught_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    if (make_ok() catch error { return 1 }) is Result.ok(value) {
        return value
    }

    return 0
}

func make_ok(): Result! {
    return Result.ok(42)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_otherwise_call_target_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-otherwise-call-target");
    let source = project.write_source(
        "payload_enum_if_is_otherwise_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    return score(true) + score(false)
}

func score(has_value: bool): i32 {
    return if (maybe_ok(has_value) otherwise { Result.ok(2) }) is Result.ok(value) {
        value
    } else {
        0
    }
}

func maybe_ok(has_value: bool): Result? {
    if has_value {
        return Result.ok(40)
    }
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_otherwise_call_target_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-otherwise-call-target");
    let source = project.write_source(
        "payload_enum_match_otherwise_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    return match (maybe_ok() otherwise { Result.ok(42) }) {
        Result.ok(value) { value }
        _ { 0 }
    }
}

func maybe_ok(): Result? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_otherwise_pattern_target_fallback_locals_before_join_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-otherwise-target-fallback-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_otherwise_target_fallback_drop.nct",
        r#"use std/process.exit

struct Guard {
    code: i32
}

impl Guard {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    if (maybe_ok() otherwise {
        var guard = Guard { code: 42 }
        Result.ok(1)
    }) is Result.ok(value) {
        return value
    }

    return 0
}

func maybe_ok(): Result? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_transfers_owned_payload_from_otherwise_fallback_target_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-otherwise-owned-target");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_if_is_otherwise_owned_target.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    if (maybe_ok() otherwise { Result.ok(Payload { code: 42 }) }) is Result.ok(value) {
        return 1
    }

    return 0
}

func maybe_ok(): Result? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_moves_otherwise_fallback_local_into_pattern_target_once() {
    let project = TempProject::new("cli-run-payload-enum-otherwise-target-fallback-move");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "payload_enum_otherwise_target_fallback_move.nct",
        r#"use std/log.write

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        write("drop\n")!
        return
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    if (maybe_ok() otherwise {
        var result = Result.ok(Payload { code: 42 })
        move result
    }) is Result.ok(value) {
        return 1
    }

    return 0
}

func maybe_ok(): Result? {
    return none
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"drop\n");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_constructor_target_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-constructor-target-binding");
    let source = project.write_source(
        "payload_enum_if_is_constructor_target_binding.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    if Result.ok(42) is Result.ok(value) {
        return value
    }

    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_payload_enum_if_is_call_target_before_branch_return_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-call-target-active-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_if_is_call_target_active_drop.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    if make_ok() is Result.ok(_) {
        return 1
    }

    return 0
}

func make_ok(): Result {
    return Result.ok(Payload { code: 42 })
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_payload_enum_if_is_move_target_after_normal_completion_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-move-target-normal-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_if_is_move_target_normal_drop.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { code: 42 })
    if move result is Result.failed {
        return 1
    }

    return 7
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_skips_payload_enum_if_is_call_target_inactive_drop_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-call-target-inactive-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_if_is_call_target_inactive_drop.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    if make_failed() is Result.ok(_) {
        return 1
    }

    return 7
}

func make_failed(): Result {
    return Result.failed
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_copy_aggregate_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-copy-aggregate-binding");
    let source = project.write_source(
        "payload_enum_if_is_copy_aggregate_binding.nct",
        r#"copy struct Detail {
    code: i32
    bonus: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let ok = Result.ok(Detail { code: 40, bonus: 1 })
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): i32 {
    if result is Result.ok(value) {
        return value.code + value.bonus
    }

    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_if_is_expression_copy_aggregate_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-if-is-expression-copy-aggregate-binding");
    let source = project.write_source(
        "payload_enum_if_is_expression_copy_aggregate_binding.nct",
        r#"copy struct Detail {
    code: i32
    bonus: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let ok = Result.ok(Detail { code: 40, bonus: 1 })
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): i32 {
    let scored = if result is Result.ok(value) {
        value.code + value.bonus
    } else {
        1
    }
    return scored
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_discard_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-discard");
    let source = project.write_source(
        "payload_enum_match_discard.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let ok = Result.ok(10)
    let failed = Result.failed
    let ok_score = score(move ok)
    let failed_score = score(move failed)
    return ok_score + failed_score
}

func score(result: Result): i32 {
    match result {
        Result.ok(_) {
            return 40
        }

        _ {
            return 2
        }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_i32_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-i32-binding");
    let source = project.write_source(
        "payload_enum_match_i32_binding.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let ok = Result.ok(40)
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): i32 {
    match result {
        Result.ok(value) {
            return value
        }

        _ {
            return 2
        }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_call_target_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-call-target-binding");
    let source = project.write_source(
        "payload_enum_match_call_target_binding.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    match make_ok() {
        Result.ok(value) {
            return value
        }

        _ {
            return 0
        }
    }
}

func make_ok(): Result {
    return Result.ok(42)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_empty_variant_target_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-empty-variant-target");
    let source = project.write_source(
        "payload_enum_match_empty_variant_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    match Result.failed {
        Result.ok(_) {
            return 1
        }

        _ {
            return 42
        }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_payload_enum_match_move_target_after_normal_completion_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-move-target-normal-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_match_move_target_normal_drop.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { code: 42 })
    match move result {
        Result.ok(_) {
            let marker = 1
        }
    }

    return 7
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_copy_aggregate_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-copy-aggregate-binding");
    let source = project.write_source(
        "payload_enum_match_copy_aggregate_binding.nct",
        r#"copy struct Detail {
    code: i32
    bonus: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let ok = Result.ok(Detail { code: 40, bonus: 1 })
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): i32 {
    match result {
        Result.ok(value) {
            return value.code + value.bonus
        }

        _ {
            return 1
        }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_slice_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-slice-binding");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "payload_enum_match_slice_binding.nct",
        r#"use std/string.bytes

enum Result {
    ok(value: &[u8])
    failed
}

func main(): usize {
    let ok = Result.ok(bytes("Nocter"))
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): usize {
    match result {
        Result.ok(value) {
            return value.len()
        }

        _ {
            return 36
        }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_str_binding_exhaustive_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-str-binding-exhaustive");
    let source = project.write_source(
        "payload_enum_match_str_binding_exhaustive.nct",
        r#"enum Message {
    empty
    text(value: &str)
}

func main(): i32 {
    let text = Message.text("Nocter")
    let empty = Message.empty
    return score(move text) + score(move empty)
}

func score(message: Message): i32 {
    match message {
        Message.empty {
            return 2
        }

        Message.text(text) {
            if text.len() == 6 {
                return 40
            } else {
                return 1
            }
        }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_expression_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-expression-binding");
    let source = project.write_source(
        "payload_enum_match_expression_binding.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let ok = Result.ok(40)
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): i32 {
    return match result {
        Result.ok(value) { value }
        _ { 2 }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_expression_call_target_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-expression-call-target-binding");
    let source = project.write_source(
        "payload_enum_match_expression_call_target_binding.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    return match make_ok() {
        Result.ok(value) { value }
        _ { 0 }
    }
}

func make_ok(): Result {
    return Result.ok(42)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_expression_move_target_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-expression-move-target-binding");
    let source = project.write_source(
        "payload_enum_match_expression_move_target_binding.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(42)
    return match move result {
        Result.ok(value) { value }
        _ { 0 }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_payload_enum_match_expression_call_target_before_return_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-expression-call-target-active-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "payload_enum_match_expression_call_target_active_drop.nct",
        r#"use std/process.exit

struct Payload {
    code: i32
}

impl Payload {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    return match make_ok() {
        Result.ok(_) { 1 }
        _ { 0 }
    }
}

func make_ok(): Result {
    return Result.ok(Payload { code: 42 })
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_expression_copy_aggregate_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-expression-copy-aggregate-binding");
    let source = project.write_source(
        "payload_enum_match_expression_copy_aggregate_binding.nct",
        r#"copy struct Detail {
    code: i32
    bonus: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let ok = Result.ok(Detail { code: 40, bonus: 1 })
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): i32 {
    return match result {
        Result.ok(value) { value.code + value.bonus }
        _ { 1 }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_expression_slice_binding_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-expression-slice-binding");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "payload_enum_match_expression_slice_binding.nct",
        r#"use std/string.bytes

enum Result {
    ok(value: &[u8])
    failed
}

func main(): usize {
    let ok = Result.ok(bytes("Nocter"))
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): usize {
    return match result {
        Result.ok(value) { value.len() }
        _ { 36 }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_expression_discard_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-expression-discard");
    let source = project.write_source(
        "payload_enum_match_expression_discard.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let ok = Result.ok(40)
    let failed = Result.failed
    return score(move ok) + score(move failed)
}

func score(result: Result): i32 {
    return match result {
        Result.ok(_) { 40 }
        _ { 2 }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_discard_exhaustive_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-discard-exhaustive");
    let source = project.write_source(
        "payload_enum_match_discard_exhaustive.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let ok = Result.ok(10)
    let failed = Result.failed
    let ok_score = score(move ok)
    let failed_score = score(move failed)
    return ok_score + failed_score
}

func score(result: Result): i32 {
    match result {
        Result.ok(_) {
            return 40
        }

        Result.failed {
            return 2
        }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_discard_nonexhaustive_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-discard-nonexhaustive");
    let source = project.write_source(
        "payload_enum_match_discard_nonexhaustive.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let failed = Result.failed
    return score(move failed)
}

func score(result: Result): i32 {
    match result {
        Result.ok(_) {
            return 40
        }
    }

    return 2
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_accepts_payload_enum_match_wildcard_only_exit_code() {
    let project = TempProject::new("cli-run-payload-enum-match-wildcard-only");
    let source = project.write_source(
        "payload_enum_match_wildcard_only.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.failed
    return score(move result)
}

func score(result: Result): i32 {
    match result {
        _ {
            return 42
        }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_payloadless_if_is_exit_code() {
    let project = TempProject::new("cli-run-payloadless-if-is");
    let source = project.write_source(
        "payloadless_if_is.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.yes
    var code = 1
    if choice is Choice.yes {
        code = 21
    }
    if choose() is Choice.no {
        code = code + 21
    }
    if Choice.maybe is Choice.maybe {
        return code
    } else {
        return 1
    }
}

func choose(): Choice {
    return Choice.no
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_payloadless_match_exit_code() {
    let project = TempProject::new("cli-run-payloadless-match");
    let source = project.write_source(
        "payloadless_match.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let a = describe(Choice.yes)
    let b = describe_exhaustive(choose())
    let c = describe_no_else_then_continue(Choice.maybe)
    let d = describe_nested_branch(Choice.maybe)
    let e = describe_wildcard_only(choose())
    return a + b + c + d + e
}

func describe(choice: Choice): i32 {
    match choice {
        Choice.yes {
            return 10
        }

        Choice.no {
            return 20
        }

        _ {
            return 30
        }
    }
}

func describe_exhaustive(choice: Choice): i32 {
    match choice {
        Choice.yes {
            return 1
        }

        Choice.no {
            return 2
        }

        Choice.maybe {
            return 3
        }
    }
}

func describe_no_else_then_continue(choice: Choice): i32 {
    var code = 4
    match choice {
        Choice.yes {
            code = 5
        }
    }
    return code
}

func describe_nested_branch(choice: Choice): i32 {
    if true {
        match choice {
            Choice.yes {
                return 6
            }

            _ {
                return 7
            }
        }
    } else {
        return 8
    }
}

func describe_wildcard_only(choice: Choice): i32 {
    match choice {
        _ {
            return 9
        }
    }
}

func choose(): Choice {
    return Choice.no
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(32),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_payloadless_match_expression_body_result_exit_code() {
    let project = TempProject::new("cli-run-payloadless-match-expression-body-result");
    let source = project.write_source(
        "payloadless_match_expression_body_result.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    let a = describe(choice)
    let b = describe(choose())
    let c = describe_exhaustive(Choice.maybe)
    let d = describe_wildcard_only(Choice.yes)
    if choice is Choice.no {
        a + b + c + d + same(7)
    } else {
        1
    }
}

func describe(choice: Choice): i32 {
    match choice {
        Choice.yes { 1 }
        Choice.no { 2 }
        _ { 10 }
    }
}

func describe_exhaustive(choice: Choice): i32 {
    match choice {
        Choice.yes { 1 }
        Choice.no { 2 }
        Choice.maybe { 3 }
    }
}

func describe_wildcard_only(choice: Choice): i32 {
    match choice {
        _ { 5 }
    }
}

func choose(): Choice {
    Choice.maybe
}

func same(value: i32): i32 {
    value
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(27),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_payloadless_match_expression_with_import_alias_arms_exit_code() {
    let project = TempProject::new("cli-run-payloadless-match-expression-import-alias-arms");
    project.write_source(
        "choice.nct",
        r#"pub enum Choice {
    yes
    no
}
"#,
    );
    let source = project.write_source(
        "payloadless_match_expression_import_alias_arms.nct",
        r#"use ./choice.Choice
use ./choice.Choice as Pick

func main(): i32 {
    let choice = Pick.no
    return match choice {
        Choice.yes { 11 }
        Pick.no { 22 }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(22),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_imported_alias_payloadless_wildcard_only_match_exit_code() {
    let project = TempProject::new("cli-run-imported-alias-payloadless-wildcard-only-match");
    project.write_source(
        "choices.nct",
        r#"pub enum Choice {
    yes
    no
}

pub type PublicChoice = Choice

pub func choose(): PublicChoice {
    return Choice.no
}
"#,
    );
    let source = project.write_source(
        "payloadless_wildcard_imported_alias.nct",
        r#"use ./choices.{PublicChoice, choose}

func main(): i32 {
    let first = describe(choose())
    let second = match choose() {
        _ { 5 }
    }
    return first + second
}

func describe(choice: PublicChoice): i32 {
    match choice {
        _ {
            return 7
        }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(12),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_value_match_binding_and_assignment_exit_code() {
    let project = TempProject::new("cli-run-value-match-binding-and-assignment");
    project.write_nocter_home_file(
        "std/string.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "value_match_binding_and_assignment.nct",
        r#"use std/string.bytes

copy struct Packet {
    count: i32
    byte: u8
    size: usize
    ok: bool
}

enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    let code = match choice {
        Choice.yes { 1 }
        Choice.no { 10 }
        _ { 0 }
    }
    let byte: u8 = match choice { Choice.no { 5 } _ { 1 } }
    let size: usize = match choice { Choice.no { 7 } _ { 1 } }
    let text: &str = match choice { Choice.no { "Nocter" } _ { "Other" } }
    let data: &[u8] = match choice { Choice.no { bytes(text) } _ { bytes("x") } }
    let ok: bool = match choice { Choice.no { data.len() == 6 } _ { false } }
    var total = 0
    total = match choice { Choice.no { code } _ { 1 } }
    var packet = Packet { count: 0, byte: 0, size: 0, ok: false }
    packet.count = match choice { Choice.no { total } _ { 1 } }
    packet.byte = match choice { Choice.no { byte } _ { 1 } }
    packet.size = match choice { Choice.no { size } _ { 1 } }
    packet.ok = match choice { Choice.no { ok } _ { false } }
    return if packet.ok { packet.count + 32 } else { 1 }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_payloadless_if_expression_body_result_exit_code() {
    let project = TempProject::new("cli-run-payloadless-if-expression-body-result");
    let source = project.write_source(
        "payloadless_if_expression_body_result.nct",
        r#"func main(): i32 {
    let ready = true
    if ready {
        35
    } else {
        1
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(35),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_owned_if_is_direct_drop_payload_binding_exit_code() {
    let project = TempProject::new("cli-run-owned-if-is-direct-drop-payload-binding");
    let source = project.write_source(
        "owned_if_is_direct_drop_payload_binding.nct",
        r#"struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.ok(Detail { code: 42 })
    if move result is Result.ok(detail) {
        return detail.code
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_owned_match_direct_drop_payload_binding_exit_code() {
    let project = TempProject::new("cli-run-owned-match-direct-drop-payload-binding");
    let source = project.write_source(
        "owned_match_direct_drop_payload_binding.nct",
        r#"struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.ok(Detail { code: 43 })
    return match move result {
        Result.ok(detail) { detail.code }
        _ { 0 }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(43),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_constructor_match_direct_drop_payload_binding_exit_code() {
    let project = TempProject::new("cli-run-constructor-match-direct-drop-payload-binding");
    let source = project.write_source(
        "constructor_match_direct_drop_payload_binding.nct",
        r#"struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    return match Result.ok(Detail { code: 44 }) {
        Result.ok(detail) { detail.code }
        _ { 0 }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(44),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_owned_match_payload_binding_after_normal_completion() {
    let project = TempProject::new("cli-run-owned-match-payload-binding-normal-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "owned_match_payload_binding_normal_drop.nct",
        r#"use std/process.exit

struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.ok(Detail { code: 45 })
    match move result {
        Result.ok(detail) {
            let code = detail.code
        }
        _ {
            return 0
        }
    }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(45),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_owned_match_target_when_move_binding_arm_does_not_match() {
    let project = TempProject::new("cli-run-owned-match-unmatched-target-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "owned_match_unmatched_target_drop.nct",
        r#"use std/process.exit

struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        exit(self.code)
    }
}

enum Result {
    ok(value: Detail)
    other(value: Detail)
}

func main(): i32 {
    let result = Result.other(Detail { code: 46 })
    match move result {
        Result.ok(detail) {
            let code = detail.code
        }
        _ {
            let code = 0
        }
    }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(46),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_owned_fields_inside_active_enum_payload() {
    let project = TempProject::new("cli-run-active-enum-payload-owned-field-drop");
    write_process_exit_home(&project);
    let source = project.write_source(
        "active_enum_payload_owned_field_drop.nct",
        r#"use std/process.exit

struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        exit(self.code)
    }
}

struct Wrapper {
    detail: Detail
}

enum Result {
    ok(value: Wrapper)
    failed
}

func main(): i32 {
    let result = Result.ok(Wrapper { detail: Detail { code: 49 } })
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(49),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_moves_and_drops_recursive_enum_payload_binding() {
    let project = TempProject::new("cli-run-recursive-enum-payload-move-binding");
    write_process_exit_home(&project);
    let source = project.write_source(
        "recursive_enum_payload_move_binding.nct",
        r#"use std/process.exit

struct Detail {
    code: i32
}

impl Detail {
    drop &+self {
        exit(self.code)
    }
}

struct Wrapper {
    detail: Detail
}

enum Result {
    ok(value: Wrapper)
    failed
}

func main(): i32 {
    let result = Result.ok(Wrapper { detail: Detail { code: 50 } })
    match move result {
        Result.ok(wrapper) {
            let code = wrapper.detail.code
        }
        _ {
            return 0
        }
    }
    return 1
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(50),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_owns_and_moves_indirect_fixed_array_enum_payloads() {
    let project = TempProject::new("cli-run-indirect-fixed-array-enum-payload-ownership");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "indirect_fixed_array_enum_payload_ownership.nct",
        r#"use std/log.write

struct File {
    name: &str
}

impl File {
    drop &+self {
        write(self.name)!
        return
    }
}

enum Result {
    ok(value: [File; 2])
    other(value: [File; 2])
    failed
}

enum Outcome<T> {
    value(payload: T)
    empty
}

func scope_cleanup(): void {
    let result = Result.ok([File { name: "a" }, File { name: "b" }])
    return
}

func move_binding_cleanup(): void {
    let result = Result.ok([File { name: "c" }, File { name: "d" }])
    match move result {
        Result.ok(files) {
            let length = 2
        }
        _ {
            return
        }
    }
    return
}

func unmatched_target_cleanup(): void {
    let result = Result.other([File { name: "e" }, File { name: "f" }])
    match move result {
        Result.ok(files) {
            return
        }
        _ {
            let length = 2
        }
    }
    return
}

func moved_constructor_argument_cleanup(): void {
    let files: [File; 2] = [File { name: "g" }, File { name: "h" }]
    let result = Result.ok(move files)
    return
}

func generic_payload_cleanup(): void {
    let outcome: Outcome<[File; 2]> = Outcome.value([File { name: "i" }, File { name: "j" }])
    match move outcome {
        Outcome.value(files) {
            let length = 2
        }
        _ {
            return
        }
    }
    return
}

func main(): i32 {
    scope_cleanup()
    move_binding_cleanup()
    unmatched_target_cleanup()
    moved_constructor_argument_cleanup()
    generic_payload_cleanup()
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"badcfehgji");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_moves_direct_fixed_array_enum_payloads() {
    let project = TempProject::new("cli-run-direct-fixed-array-enum-payload-ownership");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "direct_fixed_array_enum_payload_ownership.nct",
        r#"use std/log.write

struct Token {
    tag: u8
}

impl Token {
    drop &+self {
        if self.tag == 1 {
            write("a")!
            return
        }
        write("b")!
        return
    }
}

enum Event {
    tokens(value: [Token; 2])
    empty
}

func main(): i32 {
    let event = Event.tokens([Token { tag: 1 }, Token { tag: 2 }])
    match move event {
        Event.tokens(tokens) {
            let length = 2
        }
        _ {
            return 1
        }
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"ba");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_transfers_fixed_array_enum_payloads_across_call_boundaries() {
    let project = TempProject::new("cli-run-fixed-array-enum-payload-call-boundaries");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "fixed_array_enum_payload_call_boundaries.nct",
        r#"use std/log.write

struct File {
    name: &str
}

impl File {
    drop &+self {
        write(self.name)!
        return
    }
}

enum Result {
    ok(value: [File; 2])
    failed
}

func make(first: &str, second: &str): Result {
    return Result.ok([File { name: first }, File { name: second }])
}

func maybe(): Result? {
    return none
}

func fallible(): Result! {
    return Result.ok([File { name: "g" }, File { name: "h" }])
}

func consume(result: Result): void {
    return
}

func main(): i32 {
    consume(make("a", "b"))
    match make("c", "d") {
        Result.ok(files) {
            let length = 2
        }
        _ {
            return 1
        }
    }
    match (maybe() otherwise {
        Result.ok([File { name: "e" }, File { name: "f" }])
    }) {
        Result.ok(files) {
            let length = 2
        }
        _ {
            return 2
        }
    }
    match fallible()! {
        Result.ok(files) {
            let length = 2
        }
        _ {
            return 3
        }
    }
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"badcfehg");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_drops_payload_enums_nested_in_aggregate_abi_surfaces() {
    let project = TempProject::new("cli-run-nested-payload-enum-aggregate-abi");
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "nested_payload_enum_aggregate_abi.nct",
        r#"use std/log.write

struct File {
    name: &str
}

impl File {
    drop &+self {
        write(self.name)!
        return
    }
}

enum Result {
    ok(value: [File; 2])
    failed
}

enum Outer {
    ok(value: Result)
    failed
}

struct Wrapper {
    prefix: i32
    result: Result
}

func make_outer(): Outer {
    return Outer.ok(Result.ok([File { name: "a" }, File { name: "b" }]))
}

func make_result(first: &str, second: &str): Result {
    return Result.ok([File { name: first }, File { name: second }])
}

func consume(outer: Outer): void {
    return
}

func replace(wrapper: &+Wrapper): void {
    wrapper.result = Result.ok([File { name: "i" }, File { name: "j" }])
}

func main(): i32 {
    consume(make_outer())
    var wrapper = Wrapper {
        prefix: 1,
        result: Result.ok([File { name: "c" }, File { name: "d" }]),
    }
    replace(&+wrapper)
    let result = Result.ok([File { name: "e" }, File { name: "f" }])
    let moved_wrapper = Wrapper { prefix: 2, result: move result }
    let call_wrapper = Wrapper { prefix: 3, result: make_result("g", "h") }
    return 0
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"badchgfeji");
    assert!(output.stderr.is_empty());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_does_not_drop_uninitialized_fixed_array_enum_success_payload() {
    let project = TempProject::new("cli-run-fixed-array-enum-payload-failure-cleanup");
    project.write_nocter_home_file(
        "std/error.nct",
        r#"pub type ErrorCode = &str
pub type Error = error

pub(nocter) primitive new_error(code: &str, message: &str): error

pub func Error.new(code: ErrorCode, message: &str): Error from code | message {
    return new_error(code, message)
}
"#,
    );
    project.write_nocter_home_file(
        "std/log.nct",
        r#"use std/io.write_text_raw

pub func write(text: &str): void! {
    write_text_raw(1, text)?
    return
}
"#,
    );
    project.write_nocter_home_file(
        "std/io.nct",
        r#"#target: "arm64-darwin"
pub(nocter) primitive write_text_raw(fd: i32, text: &str): void!
"#,
    );
    let source = project.write_source(
        "fixed_array_enum_payload_failure_cleanup.nct",
        r#"use std/error.Error
use std/log.write

struct File {
    name: &str
}

impl File {
    drop &+self {
        write(self.name)!
        return
    }
}

enum Result {
    ok(value: [File; 2])
    failed
}

func fail(): Result! {
    return Error.new("app.failed", "failed")
}

func main(): void! {
    let guard = Result.ok([File { name: "a" }, File { name: "b" }])
    match (fail()?) {
        Result.ok(files) {
            return
        }
        _ {
            return
        }
    }
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"ba");
    assert_eq!(output.stderr, b"app.failed: failed\n");
}
