use crate::test_support::StandardRoleInput;
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::StandardDeclarationRole;
use nocter_model::{BorrowCapability, TypeKind};
use nocter_syntax::NodeKind;

use super::check_prepared_program;
use crate::test_support::{Fixture, with_standard_roles};
use crate::{
    BodyRule, CheckedControl, CheckedOperation, CleanupTarget, CleanupTiming, IterationAcquisition,
    LoopKind, ReceiverPreparation, StaticDispatch, prepare_program_checking,
};

fn check_iteration(extra: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let source = format!(
        r"
pub interface Iterator {{
    pub type Item
    pub method &+self.next(): Self.Item?
}}
{extra}
",
    );
    let fixture = Fixture::with_standard("", &source);
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
    ];
    let input = fixture.input(false);
    let input = with_standard_roles(input, roles);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn collection_iteration_freezes_each_source_mode_without_exact_size_evidence() {
    let output = check_iteration(
        r"
struct Source {}
struct ReadIter {}
struct WriteIter {}
struct OwnedIter {}
instance Source {
    pub operator (...&self): ReadIter { return ReadIter {} }
    pub operator (...&+self): WriteIter { return WriteIter {} }
    pub operator (...self): OwnedIter { return OwnedIter {} }
}
instance ReadIter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? { return none }
}
instance WriteIter {
    impl Iterator { .Item = &+i32 }
    method &+self.next(): &+i32? { return none }
}
instance OwnedIter {
    impl Iterator { .Item = i32 }
    method &+self.next(): i32? { return none }
}
func make_iterator(): OwnedIter { return OwnedIter {} }
func modes(read: Source, write: Source, owned: Source): void {
    for item in &read {}
    var writable = move write
    for item in &+writable {}
    for item in move owned {}
    for item in make_iterator() {}
    return
}
",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| body.loops().len() == 4)
        .unwrap();
    let loops = body
        .loops()
        .iter()
        .map(|(_, loop_)| loop_)
        .collect::<Vec<_>>();
    let expected = [
        (
            ReceiverPreparation::BorrowPlace(BorrowCapability::Readonly),
            true,
        ),
        (
            ReceiverPreparation::BorrowPlace(BorrowCapability::ReadWrite),
            true,
        ),
        (ReceiverPreparation::Owned, true),
        (ReceiverPreparation::Owned, false),
    ];
    for (loop_, (preparation, expands)) in loops.into_iter().zip(expected) {
        let LoopKind::For { iteration, .. } = loop_.kind() else {
            panic!("expected collection iteration")
        };
        let CheckedOperation::IteratorAcquisition(acquisition) =
            body.nodes().get(iteration.iterator()).unwrap().operation()
        else {
            panic!("iteration must own one acquisition")
        };
        assert_eq!(acquisition.source().preparation(), preparation);
        assert_eq!(
            matches!(
                acquisition.acquisition(),
                IterationAcquisition::Expansion(_)
            ),
            expands
        );
        assert!(matches!(
            iteration.next().dispatch(),
            StaticDispatch::Direct(_)
        ));
    }
}

#[test]
fn moved_direct_iterator_has_fixed_priority_over_owned_expansion() {
    let output = check_iteration(
        r"
struct Source {}
struct Fallback {}
instance Source {
    pub operator (...self): Fallback { return Fallback {} }
}
instance Source {
    impl Iterator { .Item = i32 }
    method &+self.next(): i32? { return none }
}
instance Fallback {
    impl Iterator { .Item = i32 }
    method &+self.next(): i32? { return none }
}
func visit(source: Source): void {
    for item in move source {}
    return
}
",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| body.loops().len() == 1)
        .unwrap();
    let (_, loop_) = body.loops().iter().next().unwrap();
    let LoopKind::For { iteration, .. } = loop_.kind() else {
        panic!("expected collection iteration")
    };
    assert!(matches!(
        body.nodes().get(iteration.iterator()).unwrap().operation(),
        CheckedOperation::IteratorAcquisition(acquisition)
            if acquisition.acquisition() == &IterationAcquisition::Direct
    ));
}

#[test]
fn generic_iteration_freezes_structural_expansion_and_interface_dispatch() {
    let output = check_iteration(
        r"
func visit<C, I>(source: &C): void where (...&C): I, I impl Iterator {
    for item in &source {}
    return
}
",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| body.loops().len() == 1)
        .unwrap();
    let (_, loop_) = body.loops().iter().next().unwrap();
    let LoopKind::For { iteration, .. } = loop_.kind() else {
        panic!("expected collection iteration")
    };
    assert!(matches!(
        body.nodes().get(iteration.iterator()).unwrap().operation(),
        CheckedOperation::IteratorAcquisition(acquisition)
            if matches!(
                acquisition.acquisition(),
                IterationAcquisition::Expansion(selection)
                    if matches!(selection.dispatch(), StaticDispatch::StructuralRequirement(_))
            )
    ));
    assert!(matches!(
        iteration.next().dispatch(),
        StaticDispatch::InterfaceMethod { .. }
    ));
}

#[test]
fn opaque_iterator_uses_its_advertised_exact_interface_evidence() {
    let output = check_iteration(
        r"
struct Iter {}
instance Iter {
    impl Iterator { .Item = i32 }
    method &+self.next(): i32? { return none }
}
func make(): some Iterator { .Item = i32 } { return Iter {} }
func visit(): void {
    var iterator = make()
    for item in move iterator {}
    return
}
",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| body.loops().len() == 1)
        .unwrap();
    let (_, loop_) = body.loops().iter().next().unwrap();
    let LoopKind::For { iteration, .. } = loop_.kind() else {
        panic!("expected collection iteration")
    };

    assert!(matches!(
        iteration.next().dispatch(),
        StaticDispatch::OpaqueMethod { opaque, .. }
            if opaque == body.nodes().get(iteration.iterator()).unwrap().ty()
    ));
    assert!(matches!(
        output.program().types().get(iteration.item()),
        Some(TypeKind::Builtin(nocter_model::BuiltinType::I32))
    ));
}

#[test]
fn missing_acquisition_and_iterator_contracts_have_distinct_diagnostics() {
    let missing_acquisition = check_iteration(
        r"
struct Source {}
func invalid(source: Source): void {
    for item in source {}
    return
}
",
    )
    .unwrap_err();
    assert_eq!(
        missing_acquisition.rule(),
        Some(BodyRule::InvalidCollectionAcquisition)
    );
    assert_eq!(
        missing_acquisition.source_diagnostic().unwrap().code(),
        "E0404"
    );

    let missing_iterator = check_iteration(
        r"
struct Source {}
struct NotIterator {}
instance Source {
    pub operator (...&self): NotIterator { return NotIterator {} }
}
func invalid(source: Source): void {
    for item in &source {}
    return
}
",
    )
    .unwrap_err();
    assert_eq!(
        missing_iterator.rule(),
        Some(BodyRule::InvalidCollectionIterator)
    );
    assert_eq!(
        missing_iterator.source_diagnostic().unwrap().code(),
        "E0405"
    );
}

#[test]
fn bare_move_only_iterator_place_still_requires_explicit_move() {
    let error = check_iteration(
        r"
struct Iter {}
instance Iter {
    impl Iterator { .Item = i32 }
    method &+self.next(): i32? { return none }
}
func invalid(iterator: Iter): void {
    for item in iterator {}
    return
}
",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::ImplicitMove));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0371");
}

#[test]
fn loop_binding_uses_selected_associated_item_type() {
    let output = check_iteration(
        r"
struct Iter {}
instance Iter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? { return none }
}
func visit(iterator: Iter): void {
    for item in move iterator {}
    return
}
",
    )
    .unwrap();
    let program = output.program();
    let (_, body) = program
        .bodies()
        .iter()
        .find(|(_, body)| body.loops().len() == 1)
        .unwrap();
    let (_, loop_) = body.loops().iter().next().unwrap();
    let LoopKind::For { binding, iteration } = loop_.kind() else {
        panic!("expected collection iteration")
    };
    assert_eq!(body.locals().get(*binding).unwrap().ty(), iteration.item());
    assert!(matches!(
        program.types().get(iteration.item()),
        Some(TypeKind::Borrow { .. })
    ));
}

#[test]
fn break_and_return_drop_item_before_loop_owned_iterator() {
    for transfer in ["break", "return"] {
        let output = check_iteration(&format!(
            r"
struct Item {{}}
struct Iter {{}}
instance Iter {{
    impl Iterator {{ .Item = Item }}
    method &+self.next(): Item? {{ return none }}
}}
drop Item(&+self) {{ return }}
drop Iter(&+self) {{ return }}
func visit(iterator: Iter): void {{
    for item in move iterator {{
        {transfer}
    }}
    return
}}
",
        ))
        .unwrap();
        let (_, body) = output
            .program()
            .bodies()
            .iter()
            .find(|(_, body)| body.loops().len() == 1)
            .unwrap();
        let (_, loop_) = body.loops().iter().next().unwrap();
        let LoopKind::For { binding, iteration } = loop_.kind() else {
            panic!("expected collection iteration")
        };
        let transfer_node = body
            .nodes()
            .iter()
            .find_map(|(node, checked)| match checked.operation() {
                CheckedOperation::Control(CheckedControl::Break(_)) if transfer == "break" => {
                    Some(node)
                }
                CheckedOperation::Control(CheckedControl::Return(_)) if transfer == "return" => {
                    Some(node)
                }
                _ => None,
            })
            .unwrap();
        let actions = body
            .cleanups()
            .actions(transfer_node, CleanupTiming::BeforeTransfer)
            .unwrap();
        assert!(matches!(
            actions,
            [item, iterator, ..]
                if matches!(item.target(), CleanupTarget::Path(path) if path.root() == crate::PlaceRoot::Local(*binding))
                    && matches!(iterator.target(), CleanupTarget::Value { node, .. } if *node == iteration.iterator())
        ));
    }
}

#[test]
fn continue_drops_current_item_but_keeps_iterator() {
    let output = check_iteration(
        r"
struct Item {}
struct Iter {}
instance Iter {
    impl Iterator { .Item = Item }
    method &+self.next(): Item? { return none }
}
drop Item(&+self) { return }
drop Iter(&+self) { return }
func visit(iterator: Iter): void {
    for item in move iterator {
        continue
    }
    return
}
",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| body.loops().len() == 1)
        .unwrap();
    let (_, loop_) = body.loops().iter().next().unwrap();
    let LoopKind::For { binding, iteration } = loop_.kind() else {
        panic!("expected collection iteration")
    };
    let continue_node = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Control(CheckedControl::Continue(_))
            )
            .then_some(node)
        })
        .unwrap();
    let actions = body
        .cleanups()
        .actions(continue_node, CleanupTiming::BeforeTransfer)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action.target(),
        CleanupTarget::Path(path) if path.root() == crate::PlaceRoot::Local(*binding)
    )));
    assert!(!actions.iter().any(|action| matches!(
        action.target(),
        CleanupTarget::Value { node, .. } if *node == iteration.iterator()
    )));
}

#[test]
fn normal_exhaustion_drops_item_then_iterator_at_statement_end() {
    let output = check_iteration(
        r"
struct Item {}
struct Iter {}
instance Iter {
    impl Iterator { .Item = Item }
    method &+self.next(): Item? { return none }
}
drop Item(&+self) { return }
drop Iter(&+self) { return }
func visit(iterator: Iter): void {
    for item in move iterator {}
    return
}
",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| body.loops().len() == 1)
        .unwrap();
    let (_, loop_) = body.loops().iter().next().unwrap();
    let LoopKind::For { binding, iteration } = loop_.kind() else {
        panic!("expected collection iteration")
    };
    let body_actions = body
        .cleanups()
        .actions(loop_.body(), CleanupTiming::BeforeTransfer)
        .unwrap();
    assert!(body_actions.iter().any(|action| matches!(
        action.target(),
        CleanupTarget::Path(path) if path.root() == crate::PlaceRoot::Local(*binding)
    )));
    let loop_node = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Control(CheckedControl::Loop(_))
            )
            .then_some(node)
        })
        .unwrap();
    let loop_actions = body
        .cleanups()
        .actions(loop_node, CleanupTiming::AtStatementEnd)
        .unwrap();
    assert!(matches!(
        loop_actions,
        [action]
            if matches!(action.target(), CleanupTarget::Value { node, .. } if *node == iteration.iterator())
    ));
}

#[test]
fn consuming_iteration_cannot_export_borrow_from_iterator_storage() {
    let error = check_iteration(
        r"
struct Iter {}
instance Iter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? { return none }
}
func invalid(iterator: Iter, fallback: &i32): &i32 {
    for item in move iterator {
        return item
    }
    return fallback
}
",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::InvalidResultProvenance));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0395");
}

#[test]
fn loop_owned_iterator_storage_remains_live_through_the_complete_body_scope() {
    check_iteration(
        r"
struct Iter {}
instance Iter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? { return none }
}
func valid(iterator: Iter): usize {
    var visited: usize = 0
    for item in move iterator {
        visited = visited + 1
        let observed = item
        let _ = observed
    }
    return visited
}
",
    )
    .unwrap();
}

#[test]
fn loop_owned_iterator_storage_cannot_enter_an_outer_binding() {
    let error = check_iteration(
        r"
struct Iter {}
instance Iter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? { return none }
}
func invalid(iterator: Iter, fallback: &i32): &i32 {
    var selected = fallback
    for item in move iterator {
        selected = item
    }
    return selected
}
",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::InvalidStorageEscape));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0398");
}

#[test]
fn borrowed_iterator_keeps_source_loan_live_through_body() {
    let error = check_iteration(
        r"
struct Source {}
struct Iter { source: &Source }
instance Source {
    pub operator (...&self): Iter from self { return Iter { source: self } }
    pub method &+self.clear(): void { return }
}
instance Iter {
    impl Iterator { .Item = &i32 }
    method &+self.next(): &i32? from self { return none }
}
func invalid(source: Source): void {
    var owned = move source
    for item in &owned {
        owned.clear()
    }
    return
}
",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::ConflictingLoan));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0396");
}

#[test]
fn readwrite_iterator_holds_exclusive_source_loan_through_body() {
    let error = check_iteration(
        r"
struct Source {}
struct Iter { source: &+Source }
instance Source {
    pub operator (...&+self): Iter from self { return Iter { source: self } }
    pub method &+self.clear(): void { return }
}
instance Iter {
    impl Iterator { .Item = &+i32 }
    method &+self.next(): &+i32? from self { return none }
}
func invalid(source: Source): void {
    var owned = move source
    for item in &+owned {
        owned.clear()
    }
    return
}
",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::ConflictingLoan));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0396");
}

#[test]
fn nested_return_interleaves_item_and_iterator_lifetimes() {
    let output = check_iteration(
        r"
struct OuterItem {}
struct OuterIter {}
struct InnerItem {}
struct InnerIter {}
instance OuterIter {
    impl Iterator { .Item = OuterItem }
    method &+self.next(): OuterItem? { return none }
}
instance InnerIter {
    impl Iterator { .Item = InnerItem }
    method &+self.next(): InnerItem? { return none }
}
drop OuterItem(&+self) { return }
drop OuterIter(&+self) { return }
drop InnerItem(&+self) { return }
drop InnerIter(&+self) { return }
func make_inner(): InnerIter { return InnerIter {} }
func visit(outer: OuterIter): void {
    for outer_item in move outer {
        for inner_item in make_inner() {
            return
        }
    }
    return
}
",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| body.loops().len() == 2)
        .unwrap();
    let loops = body
        .loops()
        .iter()
        .map(|(_, loop_)| loop_)
        .collect::<Vec<_>>();
    let [outer, inner] = loops.as_slice() else {
        panic!("expected two loops")
    };
    let LoopKind::For {
        binding: outer_binding,
        iteration: outer_iteration,
    } = outer.kind()
    else {
        panic!("expected outer collection iteration")
    };
    let LoopKind::For {
        binding: inner_binding,
        iteration: inner_iteration,
    } = inner.kind()
    else {
        panic!("expected inner collection iteration")
    };
    let return_node = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Control(CheckedControl::Return(_))
            )
            .then_some(node)
        })
        .unwrap();
    let actions = body
        .cleanups()
        .actions(return_node, CleanupTiming::BeforeTransfer)
        .unwrap();
    assert!(matches!(
        actions,
        [inner_item, inner_iterator, outer_item, outer_iterator]
            if matches!(inner_item.target(), CleanupTarget::Path(path) if path.root() == crate::PlaceRoot::Local(*inner_binding))
                && matches!(inner_iterator.target(), CleanupTarget::Value { node, .. } if *node == inner_iteration.iterator())
                && matches!(outer_item.target(), CleanupTarget::Path(path) if path.root() == crate::PlaceRoot::Local(*outer_binding))
                && matches!(outer_iterator.target(), CleanupTarget::Value { node, .. } if *node == outer_iteration.iterator())
    ));
}

#[test]
fn consuming_iteration_transfers_source_once() {
    let error = check_iteration(
        r"
struct Iter {}
instance Iter {
    impl Iterator { .Item = i32 }
    method &+self.next(): i32? { return none }
}
func invalid(iterator: Iter): Iter {
    for item in move iterator {}
    move iterator
}
",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::UninitializedPlace));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0378");
}

#[test]
fn propagation_drops_item_before_iterator() {
    let output = check_iteration(
        r"
struct Item {}
struct Iter {}
instance Iter {
    impl Iterator { .Item = Item }
    method &+self.next(): Item? { return none }
}
drop Item(&+self) { return }
drop Iter(&+self) { return }
func produce(): i32! { return 1 }
func visit(iterator: Iter): void! {
    for item in move iterator {
        let _ = produce()?
    }
    return
}
",
    )
    .unwrap();
    let (_, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| body.loops().len() == 1)
        .unwrap();
    let (_, loop_) = body.loops().iter().next().unwrap();
    let LoopKind::For { binding, iteration } = loop_.kind() else {
        panic!("expected collection iteration")
    };
    let propagation = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Outcome(crate::CheckedOutcome::Propagate { .. })
            )
            .then_some(node)
        })
        .unwrap();
    let actions = body
        .cleanups()
        .actions(propagation, CleanupTiming::OnOutcomePropagation)
        .unwrap();
    assert!(matches!(
        actions,
        [item, iterator, ..]
            if matches!(item.target(), CleanupTarget::Path(path) if path.root() == crate::PlaceRoot::Local(*binding))
                && matches!(iterator.target(), CleanupTarget::Value { node, .. } if *node == iteration.iterator())
    ));
}
