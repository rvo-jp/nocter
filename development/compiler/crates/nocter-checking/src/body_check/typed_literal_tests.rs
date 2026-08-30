use crate::test_support::StandardRoleInput;
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableKind, LiteralShape, ParameterRole};
use nocter_model::{BuiltinType, TypeKind};
use nocter_syntax::NodeKind;
use nocter_toolchain_contract::StandardDeclarationRole;

use super::check_prepared_program;
use crate::test_support::{Fixture, with_standard_roles};
use crate::{
    AllocationSelection, ArgumentPackSegment, BodyRule, CheckedOperation, CheckedOutcome,
    CleanupTarget, CleanupTiming, IterationAcquisition, LoopKind, PlaceRoot, SpreadMode,
    StaticDispatch, prepare_program_checking,
};

fn checked(source: &str) -> crate::CheckedProgramOutput {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared).unwrap()
}

fn checked_with_iteration_standard(
    source: &str,
) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::with_standard("", source);
    let roles = vec![
        StandardRoleInput::new(
            StandardDeclarationRole::IteratorInterface,
            fixture.standard_declaration_token(NodeKind::InterfaceDeclaration, "Iterator"),
        ),
        StandardRoleInput::new(
            StandardDeclarationRole::IteratorItem,
            fixture.standard_declaration_token(NodeKind::AssociatedTypeDeclaration, "Item"),
        ),
        StandardRoleInput::new(
            StandardDeclarationRole::IteratorNextMethod,
            fixture.standard_declaration_token(NodeKind::InterfaceMethod, "next"),
        ),
        StandardRoleInput::new(
            StandardDeclarationRole::ExactSizeIteratorInterface,
            fixture.standard_declaration_token(NodeKind::InterfaceDeclaration, "ExactSizeIterator"),
        ),
        StandardRoleInput::new(
            StandardDeclarationRole::ExactSizeIteratorRemainingLenMethod,
            fixture.standard_declaration_token(NodeKind::InterfaceMethod, "remaining_len"),
        ),
    ];
    let input = fixture.input(false);
    let input = with_standard_roles(input, roles);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

fn iteration_standard(extra: &str) -> String {
    format!(
        r"
pub interface Iterator {{
    pub type Item
    pub method &+self.next(): Self.Item?
}}
pub interface ExactSizeIterator {{
    pub method &self.remaining_len(): usize
}}
struct Vec<T> {{}}
construct Vec<T> {{
    pub literal [](...items: T): Self {{ return Self {{}} }}
}}
{extra}
",
    )
}

#[test]
fn typed_literals_retain_exact_constructor_and_generic_arguments() {
    let output = checked(
        r#"
struct Vec<T> {}
construct Vec<T> {
    pub literal [](...items: T): Self { return Self {} }
}
struct Text {}
construct Text {
    pub literal ""(text: &str): Self { return Self {} }
}
func values(): Vec<i32> { Vec [1, 2, 3] }
func empty(): Vec<i32> { Vec [] }
func rendered(): Text { Text "line\nvalue" }
"#,
    );

    let program = output.program();
    let sequence_constructor = program
        .graph()
        .declarations()
        .callables()
        .iter()
        .find_map(|(id, callable)| {
            (callable.kind() == CallableKind::Literal(LiteralShape::Sequence)).then_some(id)
        })
        .unwrap();
    let string_constructor = program
        .graph()
        .declarations()
        .callables()
        .iter()
        .find_map(|(id, callable)| {
            (callable.kind() == CallableKind::Literal(LiteralShape::String)).then_some(id)
        })
        .unwrap();
    let sequences = program
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::PackLiteral(sequence) => Some((node.ty(), sequence)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sequences.len(), 2);
    assert_eq!(sequences[0].1.pack().segments().len(), 3);
    assert!(
        sequences[0]
            .1
            .pack()
            .segments()
            .iter()
            .all(|element| matches!(element, ArgumentPackSegment::Value(_)))
    );
    for (ty, sequence) in &sequences {
        assert_eq!(
            sequence.constructor().dispatch(),
            StaticDispatch::Direct(sequence_constructor)
        );
        let Some(TypeKind::Nominal { arguments, .. }) = program.types().get(*ty) else {
            panic!("typed sequence must produce its nominal owner")
        };
        assert_eq!(
            arguments.as_ref(),
            &[program.types().builtin(BuiltinType::I32)]
        );
        assert_eq!(
            sequence.constructor().generic_arguments().as_slice()[0].ty(),
            program.types().builtin(BuiltinType::I32)
        );
    }
    let string = program
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::StringLiteral {
                constructor, text, ..
            } => Some((constructor, text)),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        string.0.dispatch(),
        StaticDispatch::Direct(string_constructor)
    );
    assert_eq!(string.1.as_ref(), "line\nvalue");
}

#[test]
fn sequence_literal_body_uses_a_non_escaping_element_pack() {
    let output = checked(
        r"
struct Vec<T> {}
construct Vec<T> {
    pub literal [](...items: T): Self {
        let count: usize = items.len()
        for item in items {
            drop item
        }
        return Self {}
    }
}
func values(): Vec<i32> { Vec [1, 2, 3] }
",
    );
    let program = output.program();
    let (_, constructor) = program
        .graph()
        .declarations()
        .callables()
        .iter()
        .find(|(_, callable)| callable.kind() == CallableKind::Literal(LiteralShape::Sequence))
        .unwrap();
    let parameter = *constructor.parameters().first().unwrap();
    assert!(matches!(
        program
            .graph()
            .declarations()
            .parameters()
            .get(parameter)
            .unwrap()
            .role(),
        ParameterRole::ArgumentPack { position: 0 }
    ));
    let body = program.bodies().get(constructor.body().unwrap()).unwrap();
    assert!(body.nodes().iter().any(|(_, node)| {
        matches!(
            node.operation(),
            CheckedOperation::ArgumentPackLength(found) if *found == parameter
        )
    }));
    let literal_loop = body
        .loops()
        .iter()
        .find_map(|(_, loop_)| match loop_.kind() {
            LoopKind::ArgumentPack {
                binding,
                parameter: found,
                item,
            } if *found == parameter => Some((*binding, *item)),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        body.locals().get(literal_loop.0).unwrap().ty(),
        literal_loop.1
    );
    assert!(
        body.cleanups()
            .actions(body.root(), CleanupTiming::BeforeTransfer)
            .is_none_or(|actions| actions.iter().all(|action| {
                !matches!(
                    action.target(),
                    CleanupTarget::Path(path)
                        if path.root() == PlaceRoot::Parameter(parameter)
                )
            }))
    );
}

#[test]
fn sequence_argument_pack_rejects_ordinary_value_use() {
    let fixture = Fixture::new(
        r"
struct Vec<T> {}
construct Vec<T> {
    pub literal [](...items: T): Self {
        drop items
        return Self {}
    }
}
",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::InvalidArgumentPackUse));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0409");
}

#[test]
fn sequence_spreads_freeze_source_order_acquisition_and_iteration_contracts() {
    let output = checked_with_iteration_standard(&iteration_standard(
        r"
struct Source {}
struct RefIter {}
struct OwnedIter {}
instance Source {
    pub operator (...&self): RefIter { return RefIter {} }
    pub operator (...self): OwnedIter { return OwnedIter {} }
}
instance RefIter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? { return none }
}
instance RefIter {
    impl ExactSizeIterator
    method &self.remaining_len(): usize { 0 }
}
instance OwnedIter {
    impl Iterator { .Item = i32 }
    method &+self.next(): i32? { return none }
}
instance OwnedIter {
    impl ExactSizeIterator
    method &self.remaining_len(): usize { 0 }
}
func copy_spread(source: Source): Vec<i32> { Vec [0, ...source, 4] }
func borrow_spread(source: Source): void {
    let values = Vec<&i32> [...&source]
    drop values
    return
}
func move_spread(source: Source): Vec<i32> { Vec [...move source] }
",
    ))
    .unwrap();
    let program = output.program();
    let sequences = program
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter().map(move |(_, node)| (body, node)))
        .filter_map(|(body, node)| match node.operation() {
            CheckedOperation::PackLiteral(sequence) => Some((body, sequence)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sequences.len(), 3);
    assert!(matches!(
        sequences[0].1.pack().segments(),
        [
            ArgumentPackSegment::Value(_),
            ArgumentPackSegment::Spread {
                mode: SpreadMode::Copy,
                ..
            },
            ArgumentPackSegment::Value(_)
        ]
    ));
    assert!(matches!(
        sequences[1].1.pack().segments(),
        [ArgumentPackSegment::Spread {
            mode: SpreadMode::Borrow,
            ..
        }]
    ));
    assert!(matches!(
        sequences[2].1.pack().segments(),
        [ArgumentPackSegment::Spread {
            mode: SpreadMode::Move,
            ..
        }]
    ));
    for (body, sequence) in sequences {
        let ArgumentPackSegment::Spread {
            iteration,
            exact_size,
            ..
        } = sequence
            .pack()
            .segments()
            .iter()
            .find(|element| matches!(element, ArgumentPackSegment::Spread { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        let acquisition = body.nodes().get(iteration.iterator()).unwrap();
        assert!(matches!(
            acquisition.operation(),
            CheckedOperation::IteratorAcquisition(acquisition)
                if matches!(acquisition.acquisition(), IterationAcquisition::Expansion(_))
        ));
        assert!(matches!(
            iteration.next().dispatch(),
            StaticDispatch::Direct(_)
        ));
        assert!(matches!(exact_size.dispatch(), StaticDispatch::Direct(_)));
    }
}

#[test]
fn callable_argument_pack_uses_the_same_spread_plan_as_sequence_literals() {
    let output = checked_with_iteration_standard(&iteration_standard(
        r"
struct Source {}
struct RefIter {}
instance Source {
    pub operator (...&self): RefIter { return RefIter {} }
}
instance RefIter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? { return none }
}
instance RefIter {
    impl ExactSizeIterator
    method &self.remaining_len(): usize { 0 }
}
func discard(...items: i32): void {
    for item in items { let _ = item }
    return
}
func apply(source: Source): void {
    discard(0, ...source, 4)
    return
}
",
    ))
    .unwrap();
    let pack = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Call(call) => call.pack(),
            _ => None,
        })
        .expect("call argument pack");

    assert!(matches!(
        pack.segments(),
        [
            ArgumentPackSegment::Value(_),
            ArgumentPackSegment::Spread {
                mode: SpreadMode::Copy,
                ..
            },
            ArgumentPackSegment::Value(_),
        ]
    ));
}

#[test]
fn generic_spread_uses_only_lexical_expansion_and_iterator_evidence() {
    let output = checked_with_iteration_standard(&iteration_standard(
        r"
func collect<C, I, T>(source: &C): Vec<T> where (...&C): I, I impl Iterator { .Item = &T }, I impl ExactSizeIterator, copy T {
    Vec [...source]
}
",
    ))
    .unwrap();
    let (body, sequence) = output
        .program()
        .bodies()
        .iter()
        .find_map(|(_, body)| {
            body.nodes()
                .iter()
                .find_map(|(_, node)| match node.operation() {
                    CheckedOperation::PackLiteral(sequence) => Some((body, sequence)),
                    _ => None,
                })
        })
        .unwrap();
    let [
        ArgumentPackSegment::Spread {
            mode: SpreadMode::Copy,
            iteration,
            exact_size,
        },
    ] = sequence.pack().segments()
    else {
        panic!("generic sequence must retain its one spread")
    };
    let acquisition = body.nodes().get(iteration.iterator()).unwrap();
    assert!(matches!(
        acquisition.operation(),
        CheckedOperation::IteratorAcquisition(acquisition)
            if matches!(
                acquisition.acquisition(),
                IterationAcquisition::Expansion(selection)
                    if matches!(selection.dispatch(), StaticDispatch::StructuralRequirement { .. })
            )
    ));
    assert!(matches!(
        iteration.next().dispatch(),
        StaticDispatch::InterfaceMethod { .. }
    ));
    assert!(matches!(
        exact_size.dispatch(),
        StaticDispatch::InterfaceMethod { .. }
    ));
}

#[test]
fn direct_owned_iterator_never_falls_back_to_expansion_for_exact_size() {
    let error = checked_with_iteration_standard(&iteration_standard(
        r"
struct DirectIter {}
struct FallbackIter {}
instance DirectIter {
    pub operator (...self): FallbackIter { return FallbackIter {} }
}
instance DirectIter {
    impl Iterator { .Item = i32 }
    method &+self.next(): i32? { return none }
}
instance FallbackIter {
    impl Iterator { .Item = i32 }
    method &+self.next(): i32? { return none }
}
instance FallbackIter {
    impl ExactSizeIterator
    method &self.remaining_len(): usize { 0 }
}
func invalid(source: DirectIter): Vec<i32> { Vec [...move source] }
",
    ))
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::InvalidSpreadIterator));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0402");
}

#[test]
fn copy_spread_rejects_a_move_only_yielded_referent() {
    let error = checked_with_iteration_standard(&iteration_standard(
        r"
struct Value {}
struct Source {}
struct RefIter {}
instance Source {
    pub operator (...&self): RefIter { return RefIter {} }
}
instance RefIter {
    impl Iterator { .Item = &Value }
    method &+self.next(): &Value? { return none }
}
instance RefIter {
    impl ExactSizeIterator
    method &self.remaining_len(): usize { 0 }
}
func invalid(source: Source): Vec<Value> { Vec [...source] }
",
    ))
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::InvalidSpreadElement));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0403");
}

#[test]
fn direct_iterator_cannot_export_a_borrow_into_its_consumed_temporary() {
    let error = checked_with_iteration_standard(&iteration_standard(
        r"
struct DirectIter {}
instance DirectIter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? { return none }
}
instance DirectIter {
    impl ExactSizeIterator
    method &self.remaining_len(): usize { 0 }
}
func invalid(source: DirectIter): Vec<&i32> { Vec [...move source] }
",
    ))
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::InvalidResultProvenance));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0395");
}

#[test]
fn borrowed_spread_keeps_the_source_loan_live_through_the_result() {
    let error = checked_with_iteration_standard(&iteration_standard(
        r"
struct Source {}
struct RefIter { source: &Source }
instance Source {
    pub operator (...&self): RefIter from self { return RefIter { source: self } }
    pub method &+self.clear(): void { return }
}
instance RefIter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? from self { return none }
}
instance RefIter {
    impl ExactSizeIterator
    method &self.remaining_len(): usize { 0 }
}
func invalid(source: Source): void {
    var owned = move source
    let values = Vec<&i32> [...&owned]
    owned.clear()
    drop values
    return
}
",
    ))
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::ConflictingLoan));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0396");
}

#[test]
fn spread_iterator_is_cleaned_when_a_later_element_propagates() {
    let output = checked_with_iteration_standard(&iteration_standard(
        r"
struct Source {}
struct RefIter {}
instance Source {
    pub operator (...&self): RefIter { return RefIter {} }
}
instance RefIter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? { return none }
}
instance RefIter {
    impl ExactSizeIterator
    method &self.remaining_len(): usize { 0 }
}
drop RefIter(&+self) { return }
func build(source: Source, input: i32!): Vec<i32>! {
    Vec [0, ...source, move input?]
}
",
    ))
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| {
            body.nodes()
                .iter()
                .any(|(_, node)| matches!(node.operation(), CheckedOperation::PackLiteral(_)))
        })
        .unwrap();
    let iterator = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::IteratorAcquisition(_)
            )
            .then_some(node)
        })
        .unwrap();
    let propagation = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Outcome(CheckedOutcome::Propagate { .. })
            )
            .then_some(node)
        })
        .unwrap();
    let actions = body
        .cleanups()
        .actions(propagation, CleanupTiming::OnOutcomePropagation)
        .unwrap();
    assert!(actions.iter().any(|action| {
        matches!(action.target(), CleanupTarget::Value { node, .. } if *node == iterator)
    }));
}

#[test]
fn bare_string_expression_is_a_static_readonly_str() {
    let output = checked("func text(): &str { \"plain\\ntext\" }\n");
    let program = output.program();
    let text = program
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Constant(crate::ConstantValue::Text(text)) => Some((node.ty(), text)),
            _ => None,
        })
        .unwrap();
    assert_eq!(text.1.as_ref(), "plain\ntext");
    assert!(matches!(
        program.types().get(text.0),
        Some(TypeKind::Borrow { referent, .. })
            if *referent == program.types().builtin(BuiltinType::Str)
    ));
}

#[test]
fn explicit_literal_allocation_uses_a_validated_standard_role_place() {
    let fixture = Fixture::with_standard(
        "",
        r"
pub struct Allocator { state: usize
    kind: usize }
struct Vec<T> {}
construct Vec<T> {
    pub literal [](...items: T): Self { return Self {} }
}
func values(allocator: &+Allocator): Vec<i32> {
    Vec [1, 2] using allocator
}
",
    );
    let role = StandardRoleInput::new(
        StandardDeclarationRole::AbortingAllocator,
        fixture.standard_declaration_token(NodeKind::StructDeclaration, "Allocator"),
    );
    let input = fixture.input(false);
    let input = with_standard_roles(input, vec![role]);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let program = output.program();
    let (body, allocation) = program
        .bodies()
        .iter()
        .find_map(|(_, body)| {
            body.nodes().iter().find_map(|(_, node)| {
                let CheckedOperation::PackLiteral(sequence) = node.operation() else {
                    return None;
                };
                matches!(sequence.allocation(), AllocationSelection::Explicit(_))
                    .then_some((body, sequence.allocation()))
            })
        })
        .unwrap();
    let AllocationSelection::Explicit(allocator) = allocation else {
        panic!("using must retain an explicit allocation operand")
    };
    assert!(matches!(
        body.nodes()
            .iter()
            .find(|(node, _)| *node == allocator)
            .map(|(_, node)| node.operation()),
        Some(CheckedOperation::Place(_))
    ));
}

#[test]
fn explicit_literal_allocation_rejects_an_untrusted_nominal() {
    let fixture = Fixture::with_standard(
        "",
        r"
pub struct Allocator { state: usize
    kind: usize }
struct Untrusted {}
struct Vec<T> {}
construct Vec<T> {
    pub literal [](...items: T): Self { return Self {} }
}
func values(allocator: &+Untrusted): Vec<i32> {
    Vec [1] using allocator
}
",
    );
    let role = StandardRoleInput::new(
        StandardDeclarationRole::AbortingAllocator,
        fixture.standard_declaration_token(NodeKind::StructDeclaration, "Allocator"),
    );
    let input = fixture.input(false);
    let input = with_standard_roles(input, vec![role]);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::InvalidAllocationContext));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0399");
}
