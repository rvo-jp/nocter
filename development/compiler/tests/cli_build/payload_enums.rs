use super::*;

#[test]
fn build_command_tracks_partial_payload_construction_across_value_boundaries() {
    let project = TempProject::new("cli-build-partial-payload-construction-boundaries");
    let source = project.write_source(
        "partial_payload_construction_boundaries.nct",
        r#"struct File {
    code: i32
}

impl File {
    drop &+self {
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

enum Status {
    value(code: i32)
    empty
}

struct PlainWrapper {
    status: Status
}

func make_file(): File! {
    return File { code: 22 }
}

func consume(result: Result): i32 {
    return 1
}

func construct_argument(): i32! {
    return consume(Result.ok([File { code: 20 }, make_file()?]))
}

func construct_return(): Result! {
    return Result.ok([File { code: 20 }, make_file()?])
}

func replace_local(): i32! {
    var result: Result = Result.failed
    result = Result.ok([File { code: 20 }, make_file()?])
    return 1
}

func construct_nested_payload(): Outer! {
    return Outer.ok(Result.ok([File { code: 20 }, make_file()?]))
}

func construct_struct_field(): Wrapper! {
    return Wrapper {
        prefix: 1,
        result: Result.ok([File { code: 20 }, make_file()?]),
    }
}

func consume_wrapper(wrapper: Wrapper): i32 {
    return 1
}

func construct_struct_argument(): i32! {
    return consume_wrapper(Wrapper {
        prefix: 1,
        result: Result.ok([File { code: 20 }, make_file()?]),
    })
}

func replace_borrowed_struct_field(wrapper: &+Wrapper): i32! {
    wrapper.result = Result.ok([File { code: 20 }, make_file()?])
    return 1
}

func construct_plain_struct_field(): PlainWrapper {
    return PlainWrapper { status: Status.value(1) }
}

func replace_struct_field(): i32! {
    var wrapper = Wrapper { prefix: 1, result: Result.failed }
    wrapper.result = Result.ok([File { code: 20 }, make_file()?])
    return 1
}

func main(): i32 {
    construct_argument()!
    construct_return()!
    replace_local()!
    let outer = construct_nested_payload()!
    var wrapper = construct_struct_field()!
    construct_struct_argument()!
    replace_borrowed_struct_field(&+wrapper)!
    var plain_wrapper = construct_plain_struct_field()
    plain_wrapper.status = Status.value(2)
    replace_struct_field()!
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_lowers_trailing_void_if_is_and_match_before_implicit_return() {
    let project = TempProject::new("cli-build-trailing-void-if-is-match-implicit-return");
    let source = project.write_source(
        "trailing_void_if_is_match_implicit_return.nct",
        r#"enum Choice {
    yes
    no
}

func main(): void {
    run_if_is(Choice.yes)
    run_match(Choice.no)
}

func run_if_is(choice: Choice): void {
    if choice is Choice.yes {
        effect()
    }
}

func run_match(choice: Choice): void {
    match choice {
        Choice.yes {
            effect()
        }

        Choice.no {
            effect()
        }
    }
}

func effect(): void {
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_binding_tag_only() {
    let project = TempProject::new("cli-build-payload-enum-match-binding-tag-only");
    let source = project.write_source(
        "payload_enum_match_binding_tag_only.nct",
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
    let error = AppError.missing_path
    return describe(move error)
}

func describe(error: AppError): i32 {
    match error {
        AppError.missing_path {
            return 0
        }

        AppError.open_failed(path) {
            if path.len() == 9 {
                return 42
            } else {
                return 1
            }
        }
    }

    return 2
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_copy_aggregate_binding() {
    let project = TempProject::new("cli-build-payload-enum-match-copy-aggregate-binding");
    let source = project.write_source(
        "payload_enum_match_copy_aggregate_binding.nct",
        r#"copy struct Detail {
    code: i32
}

enum AppError {
    missing_path
    open_failed(detail: Detail)
}

func main(): i32 {
    let error = AppError.open_failed(Detail { code: 42 })
    return describe(move error)
}

func describe(error: AppError): i32 {
    match error {
        AppError.missing_path {
            return 0
        }

        AppError.open_failed(detail) {
            return detail.code
        }
    }

    return 2
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_slice_binding() {
    let project = TempProject::new("cli-build-payload-enum-match-slice-binding");
    project.write_nocter_home_file(
        "std/string/index.nct",
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
    let result = Result.ok(bytes("Nocter"))
    return score(move result)
}

func score(result: Result): usize {
    match result {
        Result.ok(value) {
            return value.len()
        }

        _ {
            return 0
        }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_discard_tag_only() {
    let project = TempProject::new("cli-build-payload-enum-match-discard-tag-only");
    let source = project.write_source(
        "payload_enum_match_discard_tag_only.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(10)
    return score(move result)
}

func score(result: Result): i32 {
    match result {
        Result.ok(_) {
            return 42
        }

        _ {
            return 1
        }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_discard_exhaustive_no_wildcard() {
    let project = TempProject::new("cli-build-payload-enum-match-discard-exhaustive");
    let source = project.write_source(
        "payload_enum_match_discard_exhaustive.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(10)
    return score(move result)
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_discard_nonexhaustive_no_wildcard() {
    let project = TempProject::new("cli-build-payload-enum-match-discard-nonexhaustive");
    let source = project.write_source(
        "payload_enum_match_discard_nonexhaustive.nct",
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
        Result.ok(_) {
            return 40
        }
    }

    return 2
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_wildcard_only() {
    let project = TempProject::new("cli-build-payload-enum-match-wildcard-only");
    let source = project.write_source(
        "payload_enum_match_wildcard_only.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(10)
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_match_wildcard_call_target() {
    let project = TempProject::new("cli-build-payload-match-wildcard-call-target");
    let source = project.write_source(
        "payload_match_wildcard_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    match make_ok() {
        _ {
            return 42
        }
    }
}

func make_ok(): Result {
    return Result.ok(10)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_match_discard_call_target() {
    let project = TempProject::new("cli-build-payload-match-discard-call-target");
    let source = project.write_source(
        "payload_match_discard_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    match make_ok() {
        Result.ok(_) {
            return 1
        }

        _ {
            return 0
        }
    }
}

func make_ok(): Result {
    return Result.ok(10)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_match_binding_call_target() {
    let project = TempProject::new("cli-build-payload-match-binding-call-target");
    let source = project.write_source(
        "payload_match_binding_call_target.nct",
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
    return Result.ok(10)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_if_is_discard_tag_only() {
    let project = TempProject::new("cli-build-payload-enum-if-is-discard-tag-only");
    let source = project.write_source(
        "payload_enum_if_is_discard_tag_only.nct",
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
    let error = AppError.missing_path
    return describe(move error)
}

func describe(error: AppError): i32 {
    if error is AppError.open_failed(_) {
        return 1
    }

    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_if_is_binding_tag_only() {
    let project = TempProject::new("cli-build-payload-enum-if-is-binding-tag-only");
    let source = project.write_source(
        "payload_enum_if_is_binding_tag_only.nct",
        r#"enum AppError {
    missing_path
    open_failed(path: &str)
}

func main(): i32 {
    let error = AppError.open_failed("input.nct")
    return describe(move error)
}

func describe(error: AppError): i32 {
    if error is AppError.open_failed(path) {
        if path.len() == 9 {
            return 42
        }
        return 1
    }

    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_if_is_copy_aggregate_binding() {
    let project = TempProject::new("cli-build-payload-enum-if-is-copy-aggregate-binding");
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
    let result = Result.ok(Detail { code: 40, bonus: 2 })
    if result is Result.ok(value) {
        return value.code + value.bonus
    }

    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_if_is_slice_binding() {
    let project = TempProject::new("cli-build-payload-enum-if-is-slice-binding");
    project.write_nocter_home_file(
        "std/string/index.nct",
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
    let result = Result.ok(bytes("Nocter"))
    if result is Result.ok(value) {
        return value.len()
    }

    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_if_is_discard_call_target() {
    let project = TempProject::new("cli-build-payload-if-is-discard-call-target");
    let source = project.write_source(
        "payload_if_is_discard_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    if make_ok() is Result.ok(_) {
        return 1
    }

    return 0
}

func make_ok(): Result {
    return Result.ok(10)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_if_is_binding_call_target() {
    let project = TempProject::new("cli-build-payload-if-is-binding-call-target");
    let source = project.write_source(
        "payload_if_is_binding_call_target.nct",
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
    return Result.ok(10)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_if_is_expression_discard_call_target() {
    let project = TempProject::new("cli-build-payload-if-is-expression-discard-call-target");
    let source = project.write_source(
        "payload_if_is_expression_discard_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    return if make_ok() is Result.ok(_) {
        1
    } else {
        0
    }
}

func make_ok(): Result {
    return Result.ok(10)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_if_is_expression_binding_call_target() {
    let project = TempProject::new("cli-build-payload-if-is-expression-binding-call-target");
    let source = project.write_source(
        "payload_if_is_expression_binding_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    return if make_ok() is Result.ok(value) {
        value
    } else {
        0
    }
}

func make_ok(): Result {
    return Result.ok(10)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_scope_drop_body_with_multi_field_payload_enum() {
    let project = TempProject::new("cli-build-scope-drop-body-multi-payload");
    let source = project.write_source(
        "scope_drop_body_boundary.nct",
        r#"struct Payload {
    value: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

struct Resource {
    value: i32
}

impl Resource {
    drop &+self {
        let result = Result.ok(Payload { value: 1 }, Payload { value: 2 })
        return
    }
}

func main(): i32 {
    let resource = Resource { value: 1 }
    return resource.value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_field_replacement_drop_body_with_multi_field_payload_enum() {
    let project = TempProject::new("cli-build-field-replacement-drop-body-multi-payload");
    let source = project.write_source(
        "field_replacement_drop_body_boundary.nct",
        r#"struct Payload {
    value: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

struct Resource {
    value: i32
}

impl Resource {
    drop &+self {
        let result = Result.ok(Payload { value: 1 }, Payload { value: 2 })
        return
    }
}

struct Holder {
    inner: Resource
}

func main(): i32 {
    var holder = Holder { inner: Resource { value: 1 } }
    holder.inner = Resource { value: 2 }
    return holder.inner.value
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payloadless_enum_equality() {
    let project = TempProject::new("cli-build-payloadless-enum-equality");
    let source = project.write_source(
        "payloadless_enum_equality.nct",
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    if Choice.yes == Choice.no {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payloadless_if_is() {
    let project = TempProject::new("cli-build-payloadless-if-is");
    let source = project.write_source(
        "payloadless_if_is.nct",
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    if choice is Choice.yes {
        return 42
    } else {
        return 1
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payloadless_match() {
    let project = TempProject::new("cli-build-payloadless-match");
    let source = project.write_source(
        "payloadless_match.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    match choose() {
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

func choose(): Choice {
    return Choice.yes
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payloadless_wildcard_only_match() {
    let project = TempProject::new("cli-build-payloadless-wildcard-only-match");
    let source = project.write_source(
        "payloadless_wildcard_only_match.nct",
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    match choose() {
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payloadless_match_expression_body_result() {
    let project = TempProject::new("cli-build-payloadless-match-expression-body-result");
    let source = project.write_source(
        "payloadless_match_expression_body_result.nct",
        r#"enum Choice {
    yes
    no
    maybe
}

func main(): i32 {
    let choice = Choice.no
    match choice {
        Choice.yes { 1 }
        Choice.no { 2 }
        _ { 3 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payloadless_match_expression_with_import_alias_arms() {
    let project = TempProject::new("cli-build-payloadless-match-expression-import-alias-arms");
    project.write_source(
        "choice/index.nct",
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
    let choice = Choice.yes
    return match choice {
        Choice.yes { 1 }
        Pick.no { 2 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_value_match_bindings_and_assignments() {
    let project = TempProject::new("cli-build-value-match-bindings-and-assignments");
    project.write_nocter_home_file(
        "std/string/index.nct",
        r#"pub(nocter) primitive bytes_from_str(value: &str): &[u8]

pub func bytes(value: &str): &[u8] {
    return bytes_from_str(value)
}
"#,
    );
    let source = project.write_source(
        "value_match_bindings_and_assignments.nct",
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

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_copy_payload_enum_construction() {
    let project = TempProject::new("cli-build-copy-payload-enum-construction");
    let source = project.write_source(
        "copy_payload_enum_construction.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(10)
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_empty_variant_construction() {
    let project = TempProject::new("cli-build-payload-enum-empty-variant-construction");
    let source = project.write_source(
        "payload_enum_empty_variant_construction.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.failed
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_single_drop_payload_construction() {
    let project = TempProject::new("cli-build-payload-enum-single-drop-payload-construction");
    let source = project.write_source(
        "payload_enum_single_drop_payload_construction.nct",
        r#"struct Payload {
    value: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(value: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { value: 10 })
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_multi_drop_payload_construction() {
    let project = TempProject::new("cli-build-payload-enum-multi-drop-payload-construction");
    let source = project.write_source(
        "payload_enum_multi_drop_payload_construction.nct",
        r#"struct Payload {
    value: i32
}

impl Payload {
    drop &+self {
        return
    }
}

enum Result {
    ok(first: Payload, second: Payload)
    failed
}

func main(): i32 {
    let result = Result.ok(Payload { value: 10 }, Payload { value: 20 })
    return 0
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_expression_binding_tag_only() {
    let project = TempProject::new("cli-build-payload-enum-match-expression-binding-tag-only");
    let source = project.write_source(
        "payload_enum_match_expression_binding_tag_only.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(10)
    return match result {
        Result.ok(value) { value }
        _ { 0 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_expression_copy_aggregate_binding() {
    let project =
        TempProject::new("cli-build-payload-enum-match-expression-copy-aggregate-binding");
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
    let result = Result.ok(Detail { code: 40, bonus: 2 })
    return match result {
        Result.ok(value) { value.code + value.bonus }
        _ { 0 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_expression_slice_binding() {
    let project = TempProject::new("cli-build-payload-enum-match-expression-slice-binding");
    project.write_nocter_home_file(
        "std/string/index.nct",
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
    let result = Result.ok(bytes("Nocter"))
    return match result {
        Result.ok(value) { value.len() }
        _ { 0 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_enum_match_expression_discard_tag_only() {
    let project = TempProject::new("cli-build-payload-enum-match-expression-discard-tag-only");
    let source = project.write_source(
        "payload_enum_match_expression_discard_tag_only.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(10)
    return match result {
        Result.ok(_) { 1 }
        _ { 0 }
    }
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_match_expression_wildcard_call_target() {
    let project = TempProject::new("cli-build-payload-match-expression-wildcard-call-target");
    let source = project.write_source(
        "payload_match_expression_wildcard_call_target.nct",
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    return match make_ok() {
        _ { 42 }
    }
}

func make_ok(): Result {
    return Result.ok(10)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}

#[test]
fn build_command_accepts_payload_match_expression_binding_call_target() {
    let project = TempProject::new("cli-build-payload-match-expression-binding-call-target");
    let source = project.write_source(
        "payload_match_expression_binding_call_target.nct",
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
    return Result.ok(10)
}
"#,
    );

    let output = nocter(&project, ["build", source.to_str().unwrap()]);
    let executable = source.with_extension("");

    assert_success(&output);
    assert_macho_executable(&executable);
}
