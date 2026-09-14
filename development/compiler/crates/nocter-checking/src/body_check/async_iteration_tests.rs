use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_syntax::NodeKind;
use nocter_toolchain_contract::StandardDeclarationRole;

use super::check_prepared_program;
use crate::test_support::{Fixture, StandardRoleInput, with_standard_roles};
use crate::{
    BodyRule, CheckedOperation, IterationAcquisition, LoopKind, ReceiverPreparation,
    prepare_program_checking,
};

fn check_async_iteration(
    extra: &str,
) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let source = format!(
        r"
pub interface AsyncIterator {{
    pub type Item
    pub async method &+self.next(): Self.Item?!
}}
{extra}
",
    );
    let fixture = Fixture::with_standard("", &source);
    let roles = vec![
        StandardRoleInput::new(
            StandardDeclarationRole::AsyncIteratorInterface,
            fixture.standard_declaration_token(NodeKind::InterfaceDeclaration, "AsyncIterator"),
        ),
        StandardRoleInput::new(
            StandardDeclarationRole::AsyncIteratorItem,
            fixture.standard_declaration_token(NodeKind::AssociatedTypeDeclaration, "Item"),
        ),
        StandardRoleInput::new(
            StandardDeclarationRole::AsyncIteratorNextMethod,
            fixture.standard_declaration_token(NodeKind::InterfaceMethod, "next"),
        ),
    ];
    let input = with_standard_roles(fixture.input(false), roles);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

const ITERATOR: &str = r"
struct Item {}
struct Source {}
instance Source {
    impl AsyncIterator { .Item = Item }
    async method &+self.next(): Item?! { return none }
}
";

#[test]
fn async_collection_iteration_freezes_owned_acquisition_and_dispatch() {
    let output = check_async_iteration(&format!(
        r"
{ITERATOR}
async func consume(source: Source): void! {{
    for await item in move source {{
        let _ = move item
    }}
    return
}}
"
    ))
    .unwrap();
    let (body_id, body) = output
        .program()
        .bodies()
        .iter()
        .find(|(_, body)| body.loops().len() == 1)
        .unwrap();
    let (_, loop_) = body.loops().iter().next().unwrap();
    let LoopKind::ForAwait { iteration, .. } = loop_.kind() else {
        panic!("expected asynchronous collection iteration")
    };
    let CheckedOperation::IteratorAcquisition(acquisition) =
        body.nodes().get(iteration.iterator()).unwrap().operation()
    else {
        panic!("async iteration must own one acquisition")
    };
    assert_eq!(
        acquisition.source().preparation(),
        ReceiverPreparation::Owned
    );
    assert_eq!(acquisition.acquisition(), &IterationAcquisition::Direct);
    assert!(iteration.failure_outer().is_empty());
    let suspension = body
        .nodes()
        .iter()
        .find_map(|(node, checked)| {
            matches!(
                checked.operation(),
                CheckedOperation::Control(crate::CheckedControl::Loop(_))
            )
            .then_some(node)
        })
        .unwrap();
    assert!(matches!(
        output
            .program()
            .loans()
            .body(body_id)
            .unwrap()
            .suspension_storage(suspension),
        Some(storage) if storage.contains(&crate::SuspensionStorage::Value(iteration.iterator()))
    ));
}

#[test]
fn async_collection_iteration_requires_a_deferred_fallible_body() {
    let immediate = check_async_iteration(&format!(
        r"
{ITERATOR}
func consume(source: Source): void! {{
    for await item in move source {{ let _ = move item }}
    return
}}
"
    ))
    .unwrap_err();
    assert_eq!(
        immediate.rule(),
        Some(BodyRule::AsyncIterationOutsideDeferredBody)
    );

    let infallible = check_async_iteration(&format!(
        r"
{ITERATOR}
async func consume(source: Source): void {{
    for await item in move source {{ let _ = move item }}
    return
}}
"
    ))
    .unwrap_err();
    assert_eq!(infallible.rule(), Some(BodyRule::InvalidOutcomeOperation));
}

#[test]
fn async_collection_iteration_rejects_sync_iterators_and_implicit_moves() {
    let wrong_source = check_async_iteration(
        r"
struct Source {}
async func consume(source: Source): void! {
    for await item in move source { let _ = move item }
    return
}
",
    )
    .unwrap_err();
    assert_eq!(
        wrong_source.rule(),
        Some(BodyRule::InvalidAsyncCollectionIterator)
    );

    let implicit_move = check_async_iteration(&format!(
        r"
{ITERATOR}
async func consume(source: Source): void! {{
    for await item in source {{ let _ = move item }}
    return
}}
"
    ))
    .unwrap_err();
    assert_eq!(implicit_move.rule(), Some(BodyRule::ImplicitMove));
}
