use super::*;

#[test]
fn indexes_indirect_aggregate_function_signature_return_type() {
    let analysis = analyze_text(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

func main(): i32 {
    return 0
}

func make(): Text {
    return Text { start: 0, len: 0, capacity: 0 }
}
"#,
    );
    let root = analysis.root_file().unwrap();
    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let signatures = index.signatures();

    assert_eq!(
        signatures.return_type(&CallTarget::same_file("make")),
        Some(&Type::Aggregate {
            layout: ValueLayout::new(24, 8),
        })
    );
    assert_eq!(
        signatures.success_return_passing(&CallTarget::same_file("make")),
        Some(ReturnPassing::IndirectPointer)
    );
}

#[test]
fn indexes_direct_aggregate_function_signature_return_type() {
    let analysis = analyze_text(
        r#"struct Allocator {
    state: usize
    kind: u64
}

func main(): i32 {
    return 0
}

func page_allocator(): Allocator {
    return Allocator { state: 0, kind: 0 }
}
"#,
    );
    let root = analysis.root_file().unwrap();
    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let signatures = index.signatures();

    assert_eq!(
        signatures.return_type(&CallTarget::same_file("page_allocator")),
        Some(&Type::DirectAggregate {
            layout: ValueLayout::new(16, 8),
            words: 2,
        })
    );
    assert_eq!(
        signatures.success_return_passing(&CallTarget::same_file("page_allocator")),
        Some(ReturnPassing::Direct { words: 2 })
    );
}

#[test]
fn indexes_aggregate_function_signature_parameter_types() {
    let analysis = analyze_text(
        r#"struct Text {
    start: usize
    len: usize
    capacity: usize
}

struct Header {
    tag: u8
    ok: bool
    code: i32
    len: usize
}

func main(): i32 {
    return 0
}

func consume(text: Text, header: Header): i32 {
    return 0
}
"#,
    );
    let root = analysis.root_file().unwrap();
    let index = FunctionIndex::new(&analysis, root.ast.span.source);
    let signatures = index.signatures();

    assert_eq!(
        signatures.parameter_types(&CallTarget::same_file("consume")),
        Some(
            vec![
                Type::Aggregate {
                    layout: ValueLayout::new(24, 8),
                },
                Type::DirectAggregate {
                    layout: ValueLayout::new(16, 8),
                    words: 2,
                },
            ]
            .as_slice()
        )
    );
    assert_eq!(
        signatures.parameter_abi_word_count(&CallTarget::same_file("consume")),
        Some(3)
    );
}

#[test]
fn lowers_direct_aggregate_borrow_parameter_signature() {
    let function = lower_named_function(
        r#"struct Allocator {
    state: usize
    kind: usize
}

func main(): i32 {
    return 0
}

func touch(allocator: &+Allocator): void {
    return
}
"#,
        "touch",
    );

    assert_eq!(
        function,
        Function {
            name: "touch".to_string(),
            target: CallTarget::same_file("touch"),
            return_type: Type::Void,
            instructions: vec![Instruction::Return],
        }
    );
}
