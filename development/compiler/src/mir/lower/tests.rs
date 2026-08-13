use super::*;
use crate::analysis::test_support::analyze_text;
use crate::ast::{Item, Stmt};
use crate::mir::{
    ComparisonOperator, LocalOrigin, Operand, OwnershipKind, ProjectionElement, Rvalue, Statement,
    ValueRepresentation,
};

#[test]
fn builds_a_scalar_field_read_from_a_copy_aggregate_parameter() {
    let (_sources, analysis) = analyze_text(
        r#"copy struct Pair {
    first: i32
    second: i32
}

func first(pair: Pair): i32 {
    return pair.first
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "first" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &function.parameters.parameters,
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("copy aggregate field return must select MIR")
    .unwrap();

    assert_eq!(
        body.locals[1].representation,
        ValueRepresentation::Aggregate
    );
    assert_eq!(body.locals[1].ownership, OwnershipKind::Copy);
    assert!(matches!(
        body.projections.as_slice(),
        [crate::mir::ProjectionPath {
            element: ProjectionElement::Field { offset: 0 },
            ..
        }]
    ));
    assert!(matches!(
        body.blocks[0].statements.as_slice(),
        [Statement::Assign {
            value: Rvalue::Use(Operand::Copy(place)),
            ..
        }] if place.projection == Some(body.projections[0].id)
    ));
}

#[test]
fn builds_parent_linked_nested_field_projections() {
    let (_sources, analysis) = analyze_text(
        r#"copy struct Pair {
    first: usize
    second: usize
}

copy struct Envelope {
    pair: Pair
    code: i32
}

func read(value: Envelope): usize {
    return value.pair.second
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "read" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &function.parameters.parameters,
        ScalarType::Usize,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("nested aggregate field return must select MIR")
    .unwrap();

    assert_eq!(body.projections.len(), 2);
    assert_eq!(body.projections[0].parent, None);
    assert_eq!(body.projections[1].parent, Some(body.projections[0].id));
    assert_eq!(
        body.projections[0].element,
        ProjectionElement::Field { offset: 0 }
    );
    assert_eq!(
        body.projections[1].element,
        ProjectionElement::Field { offset: 8 }
    );
}

#[test]
fn builds_a_copy_aggregate_call_argument_from_a_parameter_place() {
    let (_sources, analysis) = analyze_text(
        r#"copy struct Pair {
    first: i32
    second: i32
}

func read(pair: Pair): i32 {
    return pair.second
}

func forward(pair: Pair): i32 {
    return read(pair)
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "forward" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &function.parameters.parameters,
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("copy aggregate forwarding must select MIR")
    .unwrap();

    assert!(matches!(
        &body.blocks[0].terminator,
        Terminator::Call {
            arguments,
            continuation: crate::mir::CallContinuation::Return { .. },
            ..
        } if matches!(
            arguments.as_slice(),
            [crate::mir::CallArgument {
                operand: Operand::Copy(place),
                representation: ValueRepresentation::Aggregate,
                ..
            }] if *place == crate::mir::Place::local(crate::mir::LocalId::from_index(1))
        )
    ));
}

#[test]
fn builds_wide_integer_arithmetic_with_canonical_kind() {
    let (_sources, analysis) = analyze_text(
        r#"func add(left: i64, right: i64): i64 {
    return left + right
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "add" => Some(function),
            _ => None,
        })
        .unwrap();
    let scalar = ScalarType::Integer(crate::integer::IntegerType::I64);
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &function.parameters.parameters,
        scalar,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("wide integer arithmetic must select MIR")
    .unwrap();

    assert_eq!(
        body.locals[0].representation,
        ValueRepresentation::Scalar(scalar)
    );
    assert!(matches!(
        body.blocks[0].statements.as_slice(),
        [Statement::Assign {
            value: Rvalue::Binary {
                operator: crate::mir::BinaryOperator::Add,
                ..
            },
            ..
        }]
    ));
}

#[test]
fn builds_integer_shifts_with_canonical_operators() {
    let (_sources, analysis) = analyze_text(
        r#"func shift(value: i64, count: i64): i64 {
    return (value << count) >> count
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "shift" => Some(function),
            _ => None,
        })
        .unwrap();
    let scalar = ScalarType::Integer(crate::integer::IntegerType::I64);
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &function.parameters.parameters,
        scalar,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("integer shifts must select MIR")
    .unwrap();

    assert!(matches!(
        body.blocks[0].statements.as_slice(),
        [
            Statement::Assign {
                value: Rvalue::Binary {
                    operator: crate::mir::BinaryOperator::ShiftLeft,
                    ..
                },
                ..
            },
            Statement::Assign {
                value: Rvalue::Binary {
                    operator: crate::mir::BinaryOperator::ShiftRight,
                    ..
                },
                ..
            }
        ]
    ));
}

#[test]
fn builds_unary_scalar_operations() {
    let (_sources, analysis) = analyze_text(
        r#"func negative(value: i64): i64 {
    return -value
}

func inverted(value: bool): bool {
    return !value
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();

    let build = |name: &str, scalar| {
        let function = file
            .ast
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == name => Some(function),
                _ => None,
            })
            .unwrap();
        try_build_scalar_body(
            function.body.as_ref().unwrap(),
            &function.parameters.parameters,
            scalar,
            &analysis.semantic_db,
            &file.resolved,
            &file.typed_hir,
        )
        .expect("unary scalar operation must select MIR")
        .unwrap()
    };

    let negative = build(
        "negative",
        ScalarType::Integer(crate::integer::IntegerType::I64),
    );
    assert!(matches!(
        negative.blocks[0].statements.as_slice(),
        [Statement::Assign {
            value: Rvalue::Unary {
                operator: crate::mir::UnaryOperator::Negate,
                ..
            },
            ..
        }]
    ));

    let inverted = build("inverted", ScalarType::Bool);
    assert!(matches!(
        inverted.blocks[0].statements.as_slice(),
        [Statement::Assign {
            value: Rvalue::Unary {
                operator: crate::mir::UnaryOperator::LogicalNot,
                ..
            },
            ..
        }]
    ));
}

#[test]
fn builds_short_circuit_boolean_control_flow() {
    let (_sources, analysis) = analyze_text(
        r#"func both(left: bool, right: bool): bool {
    return left && right
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "both" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &function.parameters.parameters,
        ScalarType::Bool,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("short-circuit boolean expression must select MIR")
    .unwrap();

    assert_eq!(body.blocks.len(), 4);
    assert!(matches!(
        body.blocks[0].terminator,
        Terminator::Switch { .. }
    ));
    assert!(matches!(
        body.blocks[1].statements.as_slice(),
        [Statement::Assign {
            value: Rvalue::Use(Operand::Copy(_)),
            ..
        }]
    ));
    assert!(matches!(
        body.blocks[2].statements.as_slice(),
        [Statement::Assign {
            value: Rvalue::Use(Operand::Constant(crate::mir::Constant { value: 0, .. })),
            ..
        }]
    ));
    assert_eq!(body.blocks[3].terminator, Terminator::Return);
}

#[test]
fn builds_typed_control_flow_for_a_scalar_literal_body() {
    let (_sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return 42
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .unwrap();
    let block = function.body.as_ref().unwrap();
    let body = try_build_scalar_body(
        block,
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("the source shape must select MIR")
    .unwrap();

    assert_eq!(body.source_span, block.span);
    assert_eq!(body.blocks.len(), 1);
    assert_eq!(body.blocks[0].statements.len(), 1);
    assert_eq!(body.blocks[0].terminator, Terminator::Return);
    assert_eq!(validate(&body), Ok(()));
}

#[test]
fn does_not_claim_a_body_with_runtime_statements() {
    let (_sources, analysis) = analyze_text(
        r#"func main(): i32 {
    var value = 42
    value += 7
    return value
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let block = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => function.body.as_ref(),
            _ => None,
        })
        .unwrap();

    assert!(
        try_build_scalar_body(
            block,
            &[],
            ScalarType::I32,
            &analysis.semantic_db,
            &file.resolved,
            &file.typed_hir,
        )
        .is_none()
    );
}

#[test]
fn keys_straight_line_bindings_by_resolved_local_identity() {
    let (_sources, analysis) = analyze_text(
        r#"func main(): i32 {
    let value = 42
    return value
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .unwrap();
    let block = function.body.as_ref().unwrap();
    let body = try_build_scalar_body(
        block,
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("straight-line scalar bindings must select MIR")
    .unwrap();

    let Stmt::Binding(binding) = &block.statements[0] else {
        panic!("expected binding");
    };
    let symbol = file
        .resolved
        .local_symbol_id_at_name_span(binding.name_span)
        .unwrap();
    assert_eq!(body.locals.len(), 2);
    assert_eq!(body.locals[1].origin, LocalOrigin::Binding(symbol));
    assert_eq!(body.blocks[0].statements.len(), 2);
    assert!(matches!(
        body.blocks[0].statements[1],
        Statement::Assign {
            value: Rvalue::Use(Operand::Copy(_)),
            ..
        }
    ));
}

#[test]
fn makes_nested_scalar_evaluation_order_explicit_with_a_temporary() {
    let (_sources, analysis) = analyze_text(
        r#"func main(): i32 {
    return (1 + 2) * 3
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let block = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => function.body.as_ref(),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        block,
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("scalar arithmetic must select MIR")
    .unwrap();

    assert_eq!(body.locals.len(), 2);
    assert!(matches!(body.locals[1].origin, LocalOrigin::Temporary(_)));
    assert_eq!(body.blocks[0].statements.len(), 2);
    assert!(body.blocks[0].statements.iter().all(|statement| matches!(
        statement,
        Statement::Assign {
            value: Rvalue::Binary { .. },
            ..
        }
    )));
    assert_eq!(validate(&body), Ok(()));
}

#[test]
fn builds_nested_value_control_flow_as_a_scalar_operand() {
    let (_sources, analysis) = analyze_text(
        r#"func choose(condition: bool): i32 {
    return (if condition { 40 } else { 5 }) + 2
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "choose" => Some(function),
            _ => None,
        })
        .unwrap();

    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &function.parameters.parameters,
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("nested value control flow must select MIR")
    .unwrap();

    assert_eq!(body.scopes.len(), 3);
    assert_eq!(body.blocks.len(), 4);
    assert!(matches!(
        body.blocks[0].terminator,
        Terminator::Switch { .. }
    ));
    assert!(matches!(
        body.blocks[3].statements.as_slice(),
        [Statement::Assign {
            value: Rvalue::Binary { .. },
            ..
        }]
    ));
    assert_eq!(body.blocks[3].terminator, Terminator::Return);
}

#[test]
fn does_not_collapse_other_integer_parameters_into_usize_mir() {
    let (_sources, analysis) = analyze_text(
        r#"func constant(value: u16): i32 {
    return 42
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "constant" => Some(function),
            _ => None,
        })
        .unwrap();

    assert!(
        try_build_scalar_body(
            function.body.as_ref().unwrap(),
            &function.parameters.parameters,
            ScalarType::I32,
            &analysis.semantic_db,
            &file.resolved,
            &file.typed_hir,
        )
        .is_none()
    );
}

#[test]
fn builds_primitive_comparison_as_a_typed_mir_rvalue() {
    let (_sources, analysis) = analyze_text(
        r#"func choose(value: usize): i32 {
    if value < value {
        return 1
    } else {
        return 2
    }
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "choose" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &function.parameters.parameters,
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("primitive comparisons must select MIR")
    .unwrap();

    assert!(matches!(
        body.blocks[0].statements.last(),
        Some(Statement::Assign {
            value: Rvalue::Compare {
                operator: ComparisonOperator::Less,
                operand_scalar: ScalarType::Usize,
                ..
            },
            ..
        })
    ));
    assert_eq!(validate(&body), Ok(()));
}

#[test]
fn builds_scalar_tail_calls_as_identity_backed_cfg_edges() {
    let (_sources, analysis) = analyze_text(
        r#"func add_two(value: i32): i32 {
    return value + 2
}

func main(): i32 {
    return add_two(40)
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("scalar tail calls must select MIR")
    .unwrap();

    let Terminator::Call {
        callee,
        arguments,
        continuation:
            crate::mir::CallContinuation::Return {
                destination,
                target,
            },
        ..
    } = &body.blocks[0].terminator
    else {
        panic!("expected MIR call edge");
    };
    assert_eq!(
        analysis.semantic_db.definition(*callee).unwrap().kind,
        crate::semantic::DefinitionKind::Function
    );
    assert_eq!(arguments.len(), 1);
    assert_eq!(
        arguments[0].representation,
        crate::mir::ValueRepresentation::Scalar(ScalarType::I32)
    );
    assert_eq!(destination.local, body.return_local);
    assert_eq!(*target, BasicBlockId::from_index(1));
    assert_eq!(validate(&body), Ok(()));
}

#[test]
fn splits_sequential_and_nested_scalar_calls_into_ordered_cfg_edges() {
    let (_sources, analysis) = analyze_text(
        r#"func bump(value: i32): i32 {
    return value + 1
}

func main(): i32 {
    let first = bump(1)
    let second = bump(first)
    return second + bump(2)
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("ordinary scalar calls must select MIR")
    .unwrap();

    assert_eq!(body.blocks.len(), 4);
    assert!(body.loop_regions.is_empty());
    for (index, block) in body.blocks[..3].iter().enumerate() {
        let Terminator::Call {
            origin,
            continuation:
                crate::mir::CallContinuation::Return {
                    destination,
                    target,
                },
            ..
        } = block.terminator
        else {
            panic!("expected call edge at block {index}");
        };
        let crate::mir::Origin::Expression(source) = origin else {
            panic!("expected expression-backed call origin");
        };
        assert_eq!(destination.local.index(), index + 1);
        assert_eq!(target, BasicBlockId::from_index(index + 1));
        assert_eq!(analysis.semantic_db.expression(source).unwrap().id, source);
    }
    assert!(matches!(
        body.blocks[3].statements.as_slice(),
        [Statement::Assign {
            destination,
            value: Rvalue::Binary { .. },
            ..
        }] if destination.local == body.return_local
    ));
    assert_eq!(body.blocks[3].terminator, Terminator::Return);
    assert_eq!(validate(&body), Ok(()));
}

#[test]
fn builds_calls_inside_conditional_branches_as_linear_paths_to_the_join() {
    let (_sources, analysis) = analyze_text(
        r#"func answer(): i32 {
    return 42
}

func choose(condition: bool): i32 {
    if condition {
        return answer()
    } else {
        return 0
    }
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "choose" => Some(function),
            _ => None,
        })
        .unwrap();

    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &function.parameters.parameters,
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("linear branch calls must select MIR")
    .unwrap();

    assert_eq!(body.blocks.len(), 5);
    assert!(matches!(
        body.blocks[1].terminator,
        Terminator::Call {
            continuation: crate::mir::CallContinuation::Return { target, .. },
            ..
        } if target == BasicBlockId::from_index(4)
    ));
    assert_eq!(
        body.blocks[4].terminator,
        Terminator::Goto {
            target: BasicBlockId::from_index(3),
        }
    );
    assert_eq!(validate(&body), Ok(()));
}

#[test]
fn retains_lexical_scope_identity_for_branch_and_root_bindings() {
    let (_sources, analysis) = analyze_text(
        r#"func choose(condition: bool): i32 {
    if condition {
        let left = 1
    } else {
        let right = 2
    }
    let result = 3
    return result
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "choose" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &function.parameters.parameters,
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("branch-local scalar bindings must select MIR")
    .unwrap();

    assert_eq!(body.scopes.len(), 3);
    assert_eq!(body.scopes[body.root_scope.index()].parent, None);
    assert_eq!(body.scopes[1].parent, Some(body.root_scope));
    assert_eq!(body.scopes[2].parent, Some(body.root_scope));
    let binding_scope = |name: &str| {
        body.locals
            .iter()
            .find_map(|local| match local.origin {
                LocalOrigin::Binding(symbol)
                    if file.resolved.local_symbol(symbol).unwrap().name == name =>
                {
                    Some(local.scope)
                }
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(binding_scope("left"), ScopeId::from_index(1));
    assert_eq!(binding_scope("right"), ScopeId::from_index(2));
    assert_eq!(binding_scope("result"), body.root_scope);
    assert_eq!(validate(&body), Ok(()));
}

#[test]
fn builds_forced_fallible_calls_with_explicit_success_and_trap_edges() {
    let (_sources, analysis) = analyze_text(
        r#"func answer(): i32! {
    return 42
}

func main(): i32 {
    let value = answer()!
    return value
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("trapping scalar outcome calls must select MIR")
    .unwrap();

    assert_eq!(body.blocks.len(), 3);
    assert!(matches!(
        body.blocks[0].terminator,
        Terminator::Call {
            continuation: crate::mir::CallContinuation::Outcome {
                success,
                failure,
                ..
            },
            ..
        } if success == BasicBlockId::from_index(1) && failure == BasicBlockId::from_index(2)
    ));
    assert_eq!(body.blocks[2].terminator, Terminator::Trap);
    assert_eq!(validate(&body), Ok(()));
}

#[test]
fn builds_propagated_fallible_calls_with_explicit_failure_edges() {
    let (_sources, analysis) = analyze_text(
        r#"func answer(): i32! {
    return 42
}

func main(): i32! {
    let value = answer()?
    return value + 1
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body_with_return_mode(
        function.body.as_ref().unwrap(),
        &[],
        ScalarType::I32,
        ReturnMode::Fallible,
        BuildInputs {
            semantic_db: &analysis.semantic_db,
            resolved: &file.resolved,
            resolved_sources: &crate::resolve::ResolvedSources::new(),
            typed_hir: &file.typed_hir,
        },
    )
    .expect("propagating scalar outcome calls must select MIR")
    .unwrap();

    assert_eq!(body.return_mode, ReturnMode::Fallible);
    assert_eq!(body.blocks.len(), 3);
    assert!(matches!(
        body.blocks[0].terminator,
        Terminator::Call {
            continuation: crate::mir::CallContinuation::Outcome {
                success,
                failure,
                ..
            },
            ..
        } if success == BasicBlockId::from_index(1) && failure == BasicBlockId::from_index(2)
    ));
    assert_eq!(body.blocks[2].terminator, Terminator::PropagateFailure);
    assert_eq!(body.blocks[1].terminator, Terminator::Return);
    assert_eq!(validate(&body), Ok(()));
}

#[test]
fn builds_scalar_while_as_a_backedge_cfg() {
    let (_sources, analysis) = analyze_text(
        r#"func main(): i32 {
    var value = 0
    while value < 4 {
        value = value + 1
    }
    return value
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("scalar while loops must select MIR")
    .unwrap();

    assert_eq!(body.blocks.len(), 4);
    assert_eq!(
        body.loop_regions,
        vec![crate::mir::LoopRegion {
            header: BasicBlockId::from_index(1),
            condition: BasicBlockId::from_index(1),
            body: BasicBlockId::from_index(2),
            continue_target: BasicBlockId::from_index(1),
            exit: BasicBlockId::from_index(3),
        }]
    );
    assert_eq!(
        body.blocks[0].terminator,
        Terminator::Goto {
            target: BasicBlockId::from_index(1),
        }
    );
    assert!(matches!(
        body.blocks[1].terminator,
        Terminator::Switch {
            then_target,
            else_target,
            ..
        } if then_target == BasicBlockId::from_index(2)
            && else_target == BasicBlockId::from_index(3)
    ));
    assert_eq!(
        body.blocks[2].terminator,
        Terminator::Goto {
            target: BasicBlockId::from_index(1),
        }
    );
    assert_eq!(body.blocks[3].terminator, Terminator::Return);
    assert_eq!(validate(&body), Ok(()));

    let mut duplicate_region = body.clone();
    duplicate_region.loop_regions.push(body.loop_regions[0]);
    assert_eq!(
        validate(&duplicate_region),
        Err(vec![
            crate::mir::validate::ValidationError::DuplicateLoopHeader {
                header: BasicBlockId::from_index(1),
            }
        ])
    );
}

#[test]
fn maps_scalar_break_to_the_loop_exit_edge() {
    let (_sources, analysis) = analyze_text(
        r#"func main(): i32 {
    while true {
        break
    }
    return 7
}
"#,
    );
    assert!(analysis.diagnostics().is_empty());
    let file = analysis.root_file().unwrap();
    let function = file
        .ast
        .items
        .iter()
        .find_map(|item| match item {
            Item::Function(function) if function.name == "main" => Some(function),
            _ => None,
        })
        .unwrap();
    let body = try_build_scalar_body(
        function.body.as_ref().unwrap(),
        &[],
        ScalarType::I32,
        &analysis.semantic_db,
        &file.resolved,
        &file.typed_hir,
    )
    .expect("scalar loop exits must select MIR")
    .unwrap();

    assert_eq!(
        body.blocks[2].terminator,
        Terminator::Goto {
            target: BasicBlockId::from_index(3),
        }
    );
    assert_eq!(validate(&body), Ok(()));
}
