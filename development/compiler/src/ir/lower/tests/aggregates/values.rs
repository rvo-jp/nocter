use super::*;

#[test]
fn lowers_imported_function_returning_hidden_nested_aggregate_type() {
    let fixture = analyze_text_fixture_with_nocter_home_files(
        r#"use std/pack.make

func main(): i32 {
    var outer = make()
    return outer.value
}
"#,
        &[(
            "std/pack.nct",
            r#"pub copy struct Inner {
    pub tag: i32
}

pub copy struct Outer {
    pub inner: Inner
    pub value: i32
}

pub func make(): Outer {
    return Outer {
        inner: Inner { tag: 7 },
        value: 42,
    }
}
"#,
        )],
    );
    let analysis = &fixture.analysis;
    let imported_source = analysis
        .files
        .iter()
        .find(|file| {
            !file.is_root
                && file.ast.items.iter().any(|item| {
                    matches!(item, crate::ast::Item::Function(function) if function.name == "make")
                })
        })
        .map(|file| file.ast.span.source)
        .unwrap();

    let ir = lower_executable(analysis, &fixture.sources).unwrap();
    let main = ir
        .functions
        .iter()
        .find(|function| function.target == CallTarget::same_file("main"))
        .expect("expected lowered main function");

    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::CallDirectAggregate {
                destination: AggregateLocation::Slot(0),
                target: CallTarget::Imported { source, name },
                layout,
                ..
            } if *source == imported_source
                && name == "make"
                && *layout == ValueLayout::new(8, 4)
        )
    }));
    assert!(main.instructions.iter().any(|instruction| {
        matches!(
            instruction,
            Instruction::LoadAggregateI32 {
                destination: I32Location::Return,
                source: AggregateLocation::Slot(0),
                offset: 4,
            }
        )
    }));
}

#[test]
fn lowers_copy_aggregate_binding_from_copy_local() {
    let function = lower_named_function(
        r#"copy struct Pair {
    left: i32
    right: i32
}

func main(): i32 {
    return 0
}

func use_pair(): i32 {
    let source = Pair { left: 40, right: 2 }
    let target = source
    return target.left + target.right
}
"#,
        "use_pair",
    );

    assert_eq!(
        function,
        Function {
            name: "use_pair".to_string(),
            target: CallTarget::same_file("use_pair"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::ReserveAggregateSlot {
                    slot_index: 0,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 0,
                    value: i32_const(40),
                },
                Instruction::StoreAggregateI32 {
                    destination: AggregateLocation::Slot(0),
                    offset: 4,
                    value: i32_const(2),
                },
                Instruction::ReserveAggregateSlot {
                    slot_index: 1,
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::CopyAggregate {
                    destination: AggregateLocation::Slot(1),
                    source: AggregateLocation::Slot(0),
                    layout: ValueLayout::new(8, 4),
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(0),
                    source: AggregateLocation::Slot(1),
                    offset: 0,
                },
                Instruction::LoadAggregateI32 {
                    destination: I32Location::Local(1),
                    source: AggregateLocation::Slot(1),
                    offset: 4,
                },
                Instruction::AddI32 {
                    destination: I32Location::Return,
                    left: i32_local(0),
                    right: i32_local(1),
                },
                Instruction::Return,
            ],
        }
    );
}
