use super::*;
use crate::analysis::{CompileUnit, analyze_executable_compile_unit};
use crate::lexer::lex;
use crate::parser::parse;
use std::collections::HashMap;

#[test]
fn reports_reachable_unloaded_imported_call_before_ir_lowering() {
    let (sources, analysis) = analyze_text(
        r#"use std/io.print

func main(): i32 {
    print("hello")
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0435");
    assert_eq!(
        diagnostics[0].message,
        "Nocter v0 build cannot lower unloaded imported function calls yet"
    );
    assert_eq!(
        diagnostics[0].help.as_deref(),
        Some(
            "load `std/io` from the active Nocter home or use a same-file function until imported placeholder lowering is promoted"
        )
    );
    assert!(diagnostics[0].primary_span.is_some());
}

#[test]
fn does_not_report_unreachable_unloaded_imported_call() {
    let (sources, analysis) = analyze_text(
        r#"use std/io.print

func main(): i32 {
    return 0
}

func unused(): i32 {
    print("hello")
    return 1
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_str_equality() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    if "a" == "b" {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_unreachable_str_equality() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return 0
}

func unused(): bool {
    return "a" == "b"
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_payloadless_enum_equality() {
    let (sources, analysis) = analyze_text(
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

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_payloadless_if_is() {
    let (sources, analysis) = analyze_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    if choice is Choice.yes {
        return 0
    } else {
        return 1
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_payloadless_match() {
    let (sources, analysis) = analyze_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    match choice {
        Choice.yes {
            return 0
        }

        _ {
            return 1
        }
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_payloadless_wildcard_only_match() {
    let (sources, analysis) = analyze_text(
        r#"enum Choice {
    yes
    no
}

func choose(): Choice {
    return Choice.no
}

func main(): i32 {
    let value = match choose() {
        _ { 7 }
    }
    match value_choice() {
        _ {
            return value
        }
    }
}

func value_choice(): Choice {
    return Choice.yes
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_terminal_if_expression_body_result() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    let ok = true
    if ok {
        0
    } else {
        1
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_terminal_match_expression_body_result() {
    let (sources, analysis) = analyze_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    match choice {
        Choice.yes { 0 }
        _ { 1 }
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_terminal_match_expression_return_statement() {
    let (sources, analysis) = analyze_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    return match choice {
        Choice.yes { 0 }
        _ { 1 }
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_owned_payload_match_expression_direct_drop_binding() {
    let (sources, analysis) = analyze_text(
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
    let result = Result.failed
    return match move result {
        Result.ok(value) { value.code }
        _ { 0 }
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_owned_payload_match_statement_direct_drop_binding() {
    let (sources, analysis) = analyze_text(
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
    let result = Result.failed
    match move result {
        Result.ok(value) { return value.code }
        _ { return 0 }
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_move_only_fixed_array_payload_construction_and_binding() {
    let (sources, analysis) = analyze_text(
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

func main(): i32 {
    let result = Result.ok([File { code: 20 }, File { code: 22 }])
    return match move result {
        Result.ok(files) {
            var owned = move files
            return 42
        }
        _ { 0 }
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn reports_partial_move_only_fixed_array_payload_initialization_before_ir() {
    let (sources, analysis) = analyze_text(
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

func make_file(): File! {
    return File { code: 22 }
}

func main(): i32! {
    let result = Result.ok([File { code: 20 }, make_file()?])
    return 42
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0435");
    assert!(
        diagnostics[0]
            .message
            .contains("fixed array argument literals whose element initialization can exit early"),
        "{diagnostics:?}"
    );
}

#[test]
fn reports_owned_payload_match_move_binding_without_direct_drop() {
    let (sources, analysis) = analyze_text(
        r#"struct Detail {
    code: i32
}

enum Result {
    ok(value: Detail)
    failed
}

func main(): i32 {
    let result = Result.failed
    return match move result {
        Result.ok(value) { value.code }
        _ { 0 }
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
            "payload bindings outside runtime scalar/view, copy aggregate, and owned recursively droppable aggregate types in `match`"
        )),
        "{diagnostics:?}"
    );
}

#[test]
fn does_not_report_owned_if_is_direct_drop_binding_as_outer_control_move() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

enum Event {
    file(file: File)
    empty
}

func main(): i32 {
    let event = Event.file(File { fd: 1 })
    if move event is Event.file(file) {
        var moved = move file
    }
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_fully_initialized_recursive_drop_fixed_array_literal_binding() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    let files: [File; 2] = [File { fd: 1 }, File { fd: 2 }]
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_tracked_partial_initialization_recursive_drop_fixed_array_literal_binding() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32! {
    let files: [File; 2] = [File { fd: 1 }, make_file()?]
    return 0
}

func make_file(): File! {
    return File { fd: 2 }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_recursive_drop_fixed_array_literal_replacement() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var files: [File; 2] = [File { fd: 1 }, File { fd: 2 }]
    files = [File { fd: 3 }, File { fd: 4 }]
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_tracked_partial_initialization_recursive_drop_fixed_array_literal_replacement() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32! {
    var files: [File; 2] = [File { fd: 1 }, File { fd: 2 }]
    files = [File { fd: 3 }, make_file()?]
    return 0
}

func make_file(): File! {
    return File { fd: 4 }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_recursive_drop_fixed_array_local_moves_and_reinitialization() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func main(): i32 {
    var first: [File; 2] = [File { fd: 1 }, File { fd: 2 }]
    let second = move first
    first = [File { fd: 3 }, File { fd: 4 }]
    var third: [File; 2] = [File { fd: 5 }, File { fd: 6 }]
    third = move second
    drop third
    third = [File { fd: 7 }, File { fd: 8 }]
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_payload_enum_if_is_constructor_pattern_target() {
    let (sources, analysis) = analyze_text(
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

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_payload_enum_if_is_move_pattern_target() {
    let (sources, analysis) = analyze_text(
        r#"enum Result {
    ok(value: i32)
    failed
}

func main(): i32 {
    let result = Result.ok(42)
    if move result is Result.ok(value) {
        return value
    }

    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_payload_enum_match_member_pattern_target() {
    let (sources, analysis) = analyze_text(
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

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_copy_payload_enum_construction() {
    let (sources, analysis) = analyze_text(
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

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_payload_enum_empty_variant_construction() {
    let (sources, analysis) = analyze_text(
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

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_payload_enum_single_drop_payload_construction() {
    let (sources, analysis) = analyze_text(
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

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_payload_enum_multi_drop_payload_construction() {
    let (sources, analysis) = analyze_text(
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

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_scope_drop_body_with_multi_field_payload_enum() {
    let (sources, analysis) = analyze_text(
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

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_generic_scope_drop_body_with_multi_field_payload_enum() {
    let (sources, analysis) = analyze_text(
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

struct Box<T> {
    value: T
}

impl<T> Box<T> {
    drop &+self {
        let result = Result.ok(Payload { value: 1 }, Payload { value: 2 })
        return
    }
}

func main(): i32 {
    let box = Box<i32> { value: 1 }
    return box.value
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_field_replacement_drop_body_with_multi_field_payload_enum() {
    let (sources, analysis) = analyze_text(
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

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_reachable_generic_field_replacement_drop_body_with_multi_field_payload_enum() {
    let (sources, analysis) = analyze_text(
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

struct Holder<T> {
    inner: T
}

func main(): i32 {
    var holder = Holder<Resource> { inner: Resource { value: 1 } }
    holder.inner = Resource { value: 2 }
    return holder.inner.value
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_unreachable_tail_after_return() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return 0
    let bytes: [u8; 2] = [1, 2]
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_unreachable_tail_after_exhaustive_match_statement() {
    let (sources, analysis) = analyze_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    let choice = Choice.yes
    match choice {
        Choice.yes {
            return 0
        }

        Choice.no {
            return 1
        }
    }
    let stored: u16 = 0 as u16
    return 2
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_unreachable_payloadless_enum_equality() {
    let (sources, analysis) = analyze_text(
        r#"enum Choice {
    yes
    no
}

func main(): i32 {
    return 0
}

func unused(): bool {
    return Choice.yes == Choice.no
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn reports_computed_storage_only_scalar_values_before_ir_lowering() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    if (1 as u16) == (2 as u16) {
        return 1
    }
    return (1 as u16) as i32
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == "E0435")
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operations on storage-only scalar values")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conversions from computed storage-only scalar values")
    }));
}

#[test]
fn accepts_reachable_fixed_array_aggregate_field_assignment_boundary() {
    let (sources, analysis) = analyze_text(
        r#"copy struct Bag {
    values: [i32; 2]
}

func main(): i32 {
    var bag = Bag { values: [1, 2] }
    let replacement: [i32; 2] = [3, 4]
    let other = Bag { values: [5, 6] }
    bag.values = [7, 8]
    bag.values = replacement
    bag.values = make_pair()
    bag.values = make_fallible_pair()!
    bag.values = other.values
    return bag.values[0]
}

func make_pair(): [i32; 2] {
    return [9, 10]
}

func make_fallible_pair(): [i32; 2]! {
    return [11, 12]
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_fixed_array_optional_otherwise_assignment_boundary() {
    let (sources, analysis) = analyze_text(
        r#"copy struct Bag {
    tag: i32
    values: [i32; 3]
}

func main(): i32 {
    var values: [i32; 3] = [0, 0, 0]
    let fallback: [i32; 3] = [1, 2, 3]
    var bag = Bag { tag: 5, values: [0, 0, 0] }
    values = maybe_values(false) otherwise { [1, 2, 3] }
    values = maybe_values(false) otherwise { fallback }
    bag.values = maybe_values(true) otherwise { [90, 91, 92] }
    let field_success_total: i32 = sum(bag.values)
    bag.values = maybe_values(false) otherwise { make_values() }
    return sum(values) + field_success_total + sum(bag.values) + bag.tag
}

func maybe_values(flag: bool): [i32; 3]? {
    if flag {
        return [7, 8, 9]
    }
    return none
}

func make_values(): [i32; 3] {
    return [10, 11, 15]
}

func sum(values: [i32; 3]): i32 {
    return values[0] + values[1] + values[2]
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_aggregate_optional_otherwise_assignment_boundary() {
    let (sources, analysis) = analyze_text(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: i32
    second: i32
    third: i32
    fourth: i32
    fifth: i32
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
}

func main(): i32 {
    var header = Header { tag: 0, ok: false, code: 0, len: 0 }
    let fallback = Triple { first: 2, second: 8, third: 1, fourth: 1, fifth: 4 }
    var packet = Packet {
        prefix: 5,
        header: Header { tag: 3, ok: false, code: 3, len: 3 },
        triple: Triple { first: 1, second: 1, third: 1, fourth: 1, fifth: 1 },
    }
    header = maybe_header(false) otherwise { Header { tag: 1, ok: false, code: 7, len: 2 } }
    packet.header = maybe_header(true) otherwise { Header { tag: 9, ok: false, code: 90, len: 9 } }
    packet.triple = maybe_triple(false) otherwise { fallback }
    let returned = assign_with_return_fallback()
    return header_score(header) + header_score(packet.header) + triple_score(packet.triple) + returned + packet.prefix
}

func assign_with_return_fallback(): i32 {
    var header = Header { tag: 0, ok: false, code: 0, len: 0 }
    header = maybe_header(false) otherwise { return 19 }
    return header.code
}

func header_score(header: Header): i32 {
    return header.code
}

func triple_score(triple: Triple): i32 {
    return triple.second + triple.fifth
}

func maybe_header(flag: bool): Header? {
    if flag {
        return Header { tag: 4, ok: true, code: 10, len: 4 }
    }
    return none
}

func maybe_triple(flag: bool): Triple? {
    if flag {
        return Triple { first: 3, second: 30, third: 3, fourth: 3, fifth: 3 }
    }
    return none
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_aggregate_optional_otherwise_member_root_boundary() {
    let (sources, analysis) = analyze_text(
        r#"copy struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

copy struct Triple {
    first: i32
    second: i32
    third: i32
    fourth: i32
    fifth: i32
}

copy struct Packet {
    prefix: i32
    header: Header
    triple: Triple
}

func main(): i32 {
    let fallback = Packet {
        prefix: 5,
        header: Header { tag: 1, ok: false, code: 7, len: 2 },
        triple: Triple { first: 2, second: 8, third: 1, fourth: 1, fifth: 4 },
    }
    let code = (maybe_packet(false) otherwise { fallback }).header.code
    let triple = (maybe_packet(true) otherwise { fallback }).triple
    return code + triple.second + member_return_fallback()
}

func member_return_fallback(): i32 {
    let code = (maybe_packet(false) otherwise { return 11 }).header.code
    return code
}

func maybe_packet(flag: bool): Packet? {
    if flag {
        return Packet {
            prefix: 6,
            header: Header { tag: 4, ok: true, code: 10, len: 4 },
            triple: Triple { first: 3, second: 30, third: 3, fourth: 3, fifth: 3 },
        }
    }
    return none
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_generic_fixed_array_aggregate_field_boundary() {
    let (sources, analysis) = analyze_text(
        r#"copy struct Box<T> {
    values: [T; 2]
}

func main(): i32 {
    var box = Box<i32> { values: [1, 2] }
    let replacement: [i32; 2] = [3, 4]
    let other = Box<i32> { values: [20, 22] }
    box.values = [5, 6]
    box.values = replacement
    box.values = make_pair()
    box.values = other.values
    return box.values[0] + box.values[1]
}

func make_pair(): [i32; 2] {
    return [7, 8]
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn reports_reachable_fixed_array_aggregate_field_control_assignment_before_ir_lowering() {
    let (sources, analysis) = analyze_text(
        r#"copy struct Bag {
    values: [i32; 2]
}

func main(): i32 {
    var bag = Bag { values: [1, 2] }
    let replacement: [i32; 2] = [3, 4]
    let other = Bag { values: [5, 6] }
    bag.values = if true {
        replacement
    } else {
        other.values
    }
    return bag.values[0]
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "E0435"
                    && diagnostic.message
                        == "Nocter v0 build cannot lower fixed array assignments outside supported replacement values yet"
            }),
            "{diagnostics:?}"
        );
}

#[test]
fn accepts_member_rooted_slice_index_assignment_boundary() {
    let (sources, analysis) = analyze_text(
        r#"struct Buffer {
    pub bytes: &+[u8]
}

func main(): i32 {
    let holder = Buffer { bytes: buffer() }
    holder.bytes[0] = 1
    return 0
}

func buffer(): &+[u8] {
    return buffer()
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_direct_slice_binding_index_assignment_boundary() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    let bytes = buffer()
    bytes[0] = 1
    return 0
}

func buffer(): &+[u8] {
    return buffer()
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_concrete_generic_struct_literal() {
    let (sources, analysis) = analyze_text(
        r#"struct Box<T> {
    value: T
}

func main(): i32 {
    let box = Box<i32> {
        value: 42,
    }
    return box.value
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_concrete_generic_instantiation_signature() {
    let (sources, analysis) = analyze_text(
        r#"struct Box<T> {
    value: T
}

func main(): i32 {
    return make().value
}

func make(): Box<i32> {
    return Box<i32> {
        value: 42,
    }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_generic_function_with_concrete_arguments() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return identity(42)
}

func identity<T>(value: T): T {
    return value
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_nested_generic_function_with_concrete_arguments() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return forward(42)
}

func forward<T>(value: T): T {
    return identity(value)
}

func identity<T>(value: T): T {
    return value
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_generic_function_body_method_call_with_concrete_arguments() {
    let (sources, analysis) = analyze_text(
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return forward(move box)
}

func forward<T>(box: Box<T>): T {
    return (move box).into_value()
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_generic_function_with_expected_return_type() {
    let (sources, analysis) = analyze_text(
        r#"struct Marker<T> {
    code: i32
}

func main(): i32 {
    let marker: Marker<u8> = make()
    return marker.code
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_generic_function_in_catch_return_with_expected_return_type() {
    let (sources, analysis) = analyze_text(
        r#"struct Marker<T> {
    code: i32
}

func main(): i32 {
    return recover().code
}

func recover(): Marker<u8> {
    return source() catch error {
        return make()
    }
}

func source(): Marker<u8>! {
    return Marker<u8> { code: 1 }
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn reports_terminal_if_inside_catch_block_before_ir_lowering() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return source() catch error {
        if true {
            return 1
        } else {
            return 2
        }
    }
}

func source(): i32! {
    return 1
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0435");
    assert_eq!(
        diagnostics[0].message,
        "Nocter v0 build cannot lower `catch` blocks outside the v0 runtime subset yet"
    );
}

#[test]
fn reports_nested_otherwise_value_expression_before_ir_lowering() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return use_value((source() otherwise { 1 }) + 2)
}

func use_value(value: i32): i32 {
    return value
}

func source(): i32? {
    return none
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0435");
    assert_eq!(
        diagnostics[0].message,
        "Nocter v0 build cannot lower `otherwise` expressions outside direct scalar/view value, aggregate member root, aggregate argument, aggregate field initializer, binding, assignment, or return positions yet"
    );
}

#[test]
fn accepts_reachable_scalar_otherwise_direct_value_boundary() {
    let (sources, analysis) = analyze_text(
        r#"copy struct State {
    count: i32
    byte: u8
    size: usize
    ok: bool
    text: &str
}

func main(): i32 {
    let state = State {
        count: maybe_i32(false) otherwise { 2 },
        byte: maybe_u8(true) otherwise { 1 },
        size: maybe_usize(false) otherwise { 9 },
        ok: maybe_bool(true) otherwise { false },
        text: maybe_text(false) otherwise { "Nocter" },
    }
    let branch = if false {
        maybe_i32(true) otherwise { 1 }
    } else {
        maybe_i32(false) otherwise { 4 }
    }
    return combine(
        maybe_i32(true) otherwise { 1 },
        maybe_u8(false) otherwise { 3 },
        maybe_usize(true) otherwise { 1 },
        maybe_bool(false) otherwise { true },
        maybe_text(true) otherwise { "bad" },
    ) + state.count + state.byte as i32 + branch
}

func combine(count: i32, byte: u8, size: usize, ok: bool, text: &str): i32 {
    if ok && size == 8 && text.len() == 4 {
        return count + byte as i32
    }
    return 0
}

func maybe_i32(flag: bool): i32? {
    if flag { return 10 }
    return none
}

func maybe_u8(flag: bool): u8? {
    if flag { return 7 }
    return none
}

func maybe_usize(flag: bool): usize? {
    if flag { return 8 }
    return none
}

func maybe_bool(flag: bool): bool? {
    if flag { return true }
    return none
}

func maybe_text(flag: bool): &str? {
    if flag { return "lang" }
    return none
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_scalar_otherwise_assignment_boundary() {
    let (sources, analysis) = analyze_text(
        r#"copy struct State {
    count: i32
    byte: u8
    size: usize
    ok: bool
    text: &str
}

func main(): i32 {
    var count: i32 = 0
    var byte: u8 = 0
    var size: usize = 0
    var ok: bool = false
    var text: &str = "bad"
    var state = State { count: 0, byte: 0, size: 0, ok: false, text: "bad" }
    count = maybe_i32(true) otherwise { 1 }
    byte = maybe_u8(false) otherwise { 12 }
    size = maybe_usize(true) otherwise { 1 }
    ok = maybe_bool(false) otherwise { true }
    text = maybe_text(false) otherwise { "Nocter" }
    state.count = maybe_i32(false) otherwise { 5 }
    state.byte = maybe_u8(true) otherwise { 1 }
    state.size = maybe_usize(false) otherwise { 8 }
    state.ok = maybe_bool(true) otherwise { false }
    state.text = maybe_text(true) otherwise { "lang" }
    let returned = assign_with_return_fallback()
    return count + byte as i32 + state.count + state.byte as i32 + returned
}

func assign_with_return_fallback(): i32 {
    var value: i32 = 0
    value = maybe_i32(false) otherwise { return 7 }
    return value
}

func maybe_i32(flag: bool): i32? {
    if flag { return 10 }
    return none
}

func maybe_u8(flag: bool): u8? {
    if flag { return 7 }
    return none
}

func maybe_usize(flag: bool): usize? {
    if flag { return 20 }
    return none
}

func maybe_bool(flag: bool): bool? {
    if flag { return true }
    return none
}

func maybe_text(flag: bool): &str? {
    if flag { return "lang" }
    return none
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn reports_terminal_if_inside_otherwise_binding_before_ir_lowering() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    let value = source() otherwise {
        if true {
            return 1
        } else {
            return 2
        }
    }
    return value
}

func source(): i32? {
    return none
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0435");
    assert_eq!(
        diagnostics[0].message,
        "Nocter v0 build cannot lower `otherwise` fallback blocks outside the v0 binding subset yet"
    );
}

#[test]
fn accepts_reachable_generic_function_with_parameter_expected_type() {
    let (sources, analysis) = analyze_text(
        r#"struct Marker<T> {
    code: i32
}

func main(): i32 {
    return consume(make())
}

func consume(marker: Marker<u8>): i32 {
    return marker.code
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_nested_generic_function_with_parameter_expected_type() {
    let (sources, analysis) = analyze_text(
        r#"copy struct Marker<T> {
    code: i32
}

func main(): i32 {
    return consume(forward(make()))
}

func consume(marker: Marker<u8>): i32 {
    return marker.code
}

func forward<T>(value: T): T {
    return value
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn reports_unspecialized_generic_function_call_inside_reachable_specialization() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return forward(42)
}

func forward<T>(value: T): T {
    let optional = empty()
    return value
}

func empty<T>(): T? {
    return none
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0435");
    assert_eq!(
        diagnostics[0].message,
        "Nocter v0 build cannot lower generic function calls without concrete type arguments yet"
    );
}

#[test]
fn reports_stored_optional_and_fallible_locals_before_ir_lowering() {
    let (sources, analysis) = analyze_text(
        r#"type MaybeCount = i32?
type Attempt = i32!

func main(): i32 {
    let explicit_optional: MaybeCount = maybe()
    let inferred_optional = maybe()
    let explicit_fallible: Attempt = attempt()
    let inferred_fallible = attempt()
    return 0
}

func maybe(): MaybeCount {
    return none
}

func attempt(): Attempt {
    return 1
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 4, "{diagnostics:?}");
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.code == "E0435"
            && diagnostic.message
                == "Nocter v0 build cannot lower stored optional or fallible local values yet"
            && diagnostic.primary_span.is_some()
    }));
}

#[test]
fn reports_reachable_unspecialized_generic_function_call() {
    let (sources, analysis) = analyze_text(
        r#"func main(): i32 {
    let value = empty()
    return 0
}

func empty<T>(): T? {
    return none
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "E0435");
    assert_eq!(
        diagnostics[0].message,
        "Nocter v0 build cannot lower generic function calls without concrete type arguments yet"
    );
    assert_eq!(
        diagnostics[0].help.as_deref(),
        Some("make every generic parameter concrete through argument types or return context")
    );
}

#[test]
fn accepts_reachable_concrete_generic_impl_method() {
    let (sources, analysis) = analyze_text(
        r#"struct Box<T> {
    value: T
}

impl Box<i32> {
    method &self.read(): i32 {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return box.read()
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_reachable_generic_impl_method_with_concrete_receiver() {
    let (sources, analysis) = analyze_text(
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return (move box).into_value()
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn does_not_report_unreachable_generic_struct_literal() {
    let (sources, analysis) = analyze_text(
        r#"struct Box<T> {
    value: T
}

func main(): i32 {
    return 0
}

func unused(): i32 {
    let box = Box<i32> {
        value: 42,
    }
    return box.value
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_move_only_fixed_array_return_and_call_result_boundaries() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func make(first: i32, second: i32): [File; 2] {
    return [File { fd: first }, File { fd: second }]
}

func main(): i32 {
    var files: [File; 2] = make(1, 2)
    files = make(3, 4)
    make(5, 6)
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_tracked_move_only_fixed_array_return_partial_initialization() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func open(fd: i32): File! {
    return File { fd: fd }
}

func make(): [File; 2]! {
    return [File { fd: 1 }, open(2)?]
}

func main(): i32 {
    make()!
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_move_only_fixed_array_owned_parameter_boundaries() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func consume(files: [File; 2]): void {
    return
}

func forward(files: [File; 2]): void {
    consume(move files)
    return
}

func main(): i32 {
    let files: [File; 2] = [File { fd: 1 }, File { fd: 2 }]
    consume(move files)
    forward([File { fd: 3 }, File { fd: 4 }])
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn reports_move_only_fixed_array_argument_partial_initialization_before_ir_lowering() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

func open(fd: i32): File! {
    return File { fd: fd }
}

func consume(files: [File; 2]): void {
    return
}

func main(): i32! {
    consume([File { fd: 1 }, open(2)?])
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0435");
    assert_eq!(
        diagnostics[0].message,
        "Nocter v0 build cannot lower fixed array argument literals whose element initialization can exit early yet"
    );
}

#[test]
fn accepts_move_only_fixed_array_struct_field_storage_and_replacement() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Bundle {
    code: i32
    files: [File; 2]
}

func make(): [File; 2] {
    return [File { fd: 3 }, File { fd: 4 }]
}

func consume(bundle: Bundle): void {
    return
}

func main(): i32 {
    let initial: [File; 2] = [File { fd: 1 }, File { fd: 2 }]
    var bundle = Bundle { code: 42, files: move initial }
    bundle.files = [File { fd: 3 }, File { fd: 4 }]
    bundle.files = make()
    let replacement: [File; 2] = [File { fd: 5 }, File { fd: 6 }]
    bundle.files = move replacement
    consume(move bundle)
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn reports_move_only_fixed_array_struct_field_partial_initialization_before_ir_lowering() {
    let (sources, analysis) = analyze_text(
        r#"struct File {
    fd: i32
}

impl File {
    drop &+self {
        return
    }
}

struct Bundle {
    code: i32
    files: [File; 2]
}

func make(): [File; 2]! {
    return [File { fd: 1 }, File { fd: 2 }]
}

func main(): i32! {
    let bundle = Bundle { code: 42, files: make()? }
    return 0
}
"#,
    );

    let diagnostics = v0_buildability_diagnostics(&sources, &analysis);

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0435");
    assert_eq!(
        diagnostics[0].message,
        "Nocter v0 build cannot lower move-only fixed array struct fields whose initialization can exit early yet"
    );
}

fn analyze_text(text: &str) -> (SourceMap, crate::analysis::CompileUnitAnalysis) {
    let mut sources = SourceMap::new();
    let source = sources.add_source("test.nct", None, text.to_string());
    let lexed = lex(&sources, source);
    assert!(
        lexed.diagnostics.is_empty(),
        "unexpected lex diagnostics: {:?}",
        lexed.diagnostics
    );
    let parsed = parse(&sources, source, &lexed.tokens);
    assert!(
        parsed.diagnostics.is_empty(),
        "unexpected parse diagnostics: {:?}",
        parsed.diagnostics
    );
    let ast = parsed.ast.expect("expected ast");
    let unit = CompileUnit::new(ast.clone(), vec![ast], HashMap::new(), HashMap::new(), None);
    let analysis = analyze_executable_compile_unit(&sources, &unit);
    let diagnostics = analysis.diagnostics();
    assert!(
        diagnostics.is_empty(),
        "unexpected frontend diagnostics: {diagnostics:?}"
    );

    (sources, analysis)
}
