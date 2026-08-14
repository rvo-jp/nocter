use super::*;

#[test]
fn lowers_typed_string_literal_through_hidden_callable() {
    let module = lower_text(
        r#"
struct Text {
    value: &str
}

construct Text {
    pub default literal ""(text: &str): Self from text {
        return Text { value: text }
    }
}

func main(): i32 {
    let text = Text "hello"
    return 0
}
"#,
    );

    assert!(module.functions.iter().any(|function| {
        function.target == CallTarget::same_file("Text.$literal.string$704be0d8faaffc58")
    }));
    let main = module
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .unwrap();
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::CallDirectAggregate { target, arguments, .. }
            if target == &CallTarget::same_file("Text.$literal.string$704be0d8faaffc58")
                && arguments == &vec![ScalarArgument::Str(StrValue::StaticBytes(b"hello".to_vec()))]
    )));
}

#[test]
fn lowers_sequence_pack_len_and_consuming_loop() {
    let module = lower_text(
        r#"
struct Numbers {
    length: usize
}

func consume(value: i32): void {}

construct Numbers {
    pub default literal [](...items: i32): Self {
        let result = Numbers { length: items.len() }
        for item in items {
            consume(item)
        }
        return move result
    }
}

func main(): i32 {
    let values = Numbers [1, 2, 3]
    return 0
}
"#,
    );

    let target = CallTarget::same_file("Numbers.$literal.sequence$c44f4a18615cbe0a");
    let literal = module
        .functions
        .iter()
        .find(|function| function.target == target)
        .unwrap();
    assert_eq!(
        literal
            .instructions
            .iter()
            .filter(|instruction| matches!(
                instruction,
                Instruction::CallVoid { target, .. }
                    if target == &CallTarget::same_file("consume")
            ))
            .count(),
        3
    );
    assert!(literal.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::StoreAggregateUsize {
            value: UsizeValue::Const(3),
            ..
        }
    )));
}

#[test]
fn specializes_generic_sequence_literal_body_and_abi() {
    let module = lower_text(
        r#"
struct Bucket<T> {
    length: usize
}

construct Bucket<T> {
    pub default literal [](...items: T): Self {
        return Bucket<T> { length: items.len() }
    }
}

func main(): i32 {
    let values = Bucket [1, 2]
    return 0
}
"#,
    );

    let target = CallTarget::same_file("Bucket<i32>.$literal.sequence$6dc449ffe3496ec1");
    let literal = module
        .functions
        .iter()
        .find(|function| function.target == target)
        .unwrap();
    assert!(literal.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::StoreAggregateUsize {
            value: UsizeValue::Const(2),
            ..
        }
    )));
}

#[test]
fn explicit_using_installs_and_restores_allocator_context_around_construction() {
    let module = lower_text_with_nocter_home_files(
        r#"
use std/mem.page_allocator

struct Text {
    value: &str
}

construct Text {
    pub default literal ""(text: &str): Self from text {
        return Text { value: text }
    }
}

func main(): i32 {
    let arena = page_allocator()
    let text = Text "hello" using arena
    return 0
}
"#,
        &[minimal_allocator_std()],
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .unwrap();
    let context_sets = main
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| {
            matches!(instruction, Instruction::SetCurrentAllocationContext { .. }).then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(context_sets.len(), 2);
    let literal_call = main
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                Instruction::CallDirectAggregate { target, .. }
                    if call_target_name_is(target, "Text.$literal.string$704be0d8faaffc58")
            )
        })
        .unwrap();
    assert!(context_sets[0] < literal_call);
    assert!(literal_call < context_sets[1]);
}

#[test]
fn early_literal_body_return_drops_current_and_unconsumed_move_only_elements() {
    let module = lower_text(
        r#"
struct File { fd: i32 }

destruct File(&+self) { return }

struct Holder { count: usize }

construct Holder {
    pub default literal [](...items: File): Self {
        for item in items {
            return Holder { count: 1 }
        }
        return Holder { count: 0 }
    }
}

func main(): i32 {
    let first = File { fd: 1 }
    let second = File { fd: 2 }
    let holder = Holder [move first, move second]
    return 0
}
"#,
    );
    let literal = module
        .functions
        .iter()
        .find(|function| {
            function.target == CallTarget::same_file("Holder.$literal.sequence$6dc449ffe3496ec1")
        })
        .unwrap();
    assert_eq!(
        literal
            .instructions
            .iter()
            .filter(|instruction| matches!(
                instruction,
                Instruction::CallVoid { target, .. }
                    if target == &CallTarget::same_file("File.drop")
            ))
            .count(),
        2
    );
}

#[test]
fn element_failure_restores_explicit_allocator_context_before_propagation() {
    let module = lower_text_with_nocter_home_files(
        r#"
use std/mem.page_allocator

struct Numbers { count: usize }
struct Token { value: i32 }

destruct Token(&+self) { return }

construct Numbers {
    pub default literal [](...items: i32): Self {
        return Numbers { count: items.len() }
    }
}

func next(): i32! { return 2 }

func main(): i32! {
    let arena = page_allocator()
    let token = Token { value: 1 }
    let values = Numbers [1, next()?] using arena
    return 0
}
"#,
        &[minimal_allocator_std()],
    );
    let main = module
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .unwrap();
    let cleanup = main
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            Instruction::CallOutcomeI32 {
                target,
                failure_mode:
                    OutcomeFailureMode::PropagateWithCleanup { instructions, .. }
                    | OutcomeFailureMode::Handle { instructions },
                ..
            } if target == &CallTarget::same_file("next") => Some(instructions),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "element propagation should carry scope cleanup: {:?}",
                main.instructions
            )
        });
    let restore = cleanup
        .iter()
        .position(|instruction| {
            matches!(instruction, Instruction::SetCurrentAllocationContext { .. })
        })
        .expect("element propagation should restore the ambient allocator");
    let token_drop = cleanup
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                Instruction::CallVoid { target, .. }
                    if target == &CallTarget::same_file("Token.drop")
            )
        })
        .expect("element propagation should drop outer owned locals");
    assert!(restore < token_drop);
}

#[test]
fn lowers_typed_literal_directly_from_aggregate_return_context() {
    let module = lower_text(
        r#"
struct Text { value: &str }

construct Text {
    pub default literal ""(text: &str): Self from text {
        return Text { value: text }
    }
}

func make(): Text {
    return Text "hello"
}

func main(): i32 {
    let text = make()
    return 0
}
"#,
    );
    let make = module
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("make"))
        .unwrap();
    assert!(
        make.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallDirectAggregate { target, .. }
                | Instruction::TailCall { target, .. }
                if target == &CallTarget::same_file("Text.$literal.string$704be0d8faaffc58")
        )),
        "{:?}",
        make.instructions
    );
}

#[test]
fn indexes_imported_generic_literal_specialization_under_expression_target() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/items.Bucket

func main(): i32 {
    let values = Bucket [1, 2]
    return 0
}
"#,
        &[(
            "std/items/index.nct",
            r#"pub struct Bucket<T> { length: usize }

construct Bucket<T> {
    pub default literal [](...items: T): Self {
        return Bucket<T> { length: items.len() }
    }
}
"#,
        )],
    );
    let root = fixture.analysis.root_file().unwrap();
    let index = FunctionIndex::new(&fixture.analysis, root.ast.span.source);
    assert!(
        index.definitions.keys().any(|target| call_target_name_is(
            target,
            "std/items.Bucket<i32>.$literal.sequence$6dc449ffe3496ec1"
        )),
        "indexed targets: {:?}",
        index.definitions.keys().collect::<Vec<_>>()
    );
    lower_executable(&fixture.analysis, &fixture.sources).unwrap();
}

fn minimal_allocator_std() -> (&'static str, &'static str) {
    (
        "std/mem/index.nct",
        r#"
pub struct Allocator {
    state: usize
    kind: usize
}

pub func page_allocator(): Allocator {
    return Allocator { state: 7, kind: 1 }
}
"#,
    )
}
