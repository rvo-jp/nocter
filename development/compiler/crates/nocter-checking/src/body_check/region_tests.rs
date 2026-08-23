use nocter_compile_input::StandardRoleInput;
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::StandardDeclarationRole;
use nocter_model::TypeKind;
use nocter_syntax::NodeKind;

use super::check_prepared_program;
use crate::test_support::{Fixture, with_standard_roles};
use crate::{
    AllocationSelection, BodyRule, CheckedControl, CheckedOperation, CleanupTarget, CleanupTiming,
    LocalBindingKind, PlaceRoot, prepare_program_checking,
};

const REGION_STANDARD: &str = r"
pub struct Allocator { state: usize
    kind: usize
    marker: Owned }
pub struct AllocationContext { state: usize
    kind: usize }
struct Owned { value: i32 }
struct Untrusted {}
struct Vec<T> {}
construct Vec<T> {
    pub literal [](...items: T): Self { return Self {} }
}
";

fn check_region_program(
    source: &str,
) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let standard = format!("{REGION_STANDARD}\n{source}");
    let fixture = Fixture::with_standard("", &standard);
    let roles = vec![
        StandardRoleInput::new(
            StandardDeclarationRole::AbortingAllocator,
            fixture.standard_declaration_token(NodeKind::StructDeclaration, "Allocator"),
        ),
        StandardRoleInput::new(
            StandardDeclarationRole::AllocationContext,
            fixture.standard_declaration_token(NodeKind::StructDeclaration, "AllocationContext"),
        ),
    ];
    let input = fixture.input(false);
    let input = with_standard_roles(input, roles);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

#[test]
fn region_builds_a_typed_context_and_selects_current_or_explicit_allocation() {
    let output = check_region_program(
        r"
func allocate(allocator: &+Allocator): void {
    region temp using allocator {
        let current = Vec [1, 2]
        let explicit = Vec [3] using temp
        drop current
        drop explicit
    }
    return
}
",
    )
    .unwrap();
    let (body, binding, allocator, region_body) = output
        .program()
        .bodies()
        .iter()
        .find_map(|(_, body)| {
            body.nodes().iter().find_map(|(_, node)| {
                let CheckedOperation::Control(CheckedControl::Region {
                    binding,
                    allocator,
                    body: region_body,
                }) = node.operation()
                else {
                    return None;
                };
                Some((body, *binding, *allocator, *region_body))
            })
        })
        .unwrap();

    let local = body.locals().get(binding).unwrap();
    assert_eq!(local.declaration().kind(), LocalBindingKind::Region);
    assert!(matches!(
        output.program().types().get(local.ty()),
        Some(TypeKind::Nominal { arguments, .. }) if arguments.is_empty()
    ));
    assert!(matches!(
        body.nodes().get(allocator).unwrap().operation(),
        CheckedOperation::Place(_)
    ));

    let allocations = body
        .nodes()
        .iter()
        .filter_map(|(_, node)| match node.operation() {
            CheckedOperation::Sequence(sequence) => Some(sequence.allocation()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(allocations.len(), 2);
    assert!(matches!(allocations[0], AllocationSelection::CurrentRegion));
    assert!(matches!(allocations[1], AllocationSelection::Explicit(_)));
    assert!(matches!(
        body.cleanups()
            .actions(region_body, CleanupTiming::BeforeTransfer)
            .unwrap()
            .last()
            .unwrap()
            .target(),
        CleanupTarget::Region { binding: released, parent }
            if *released == binding && *parent == allocator
    ));
}

#[test]
fn region_rejects_an_untrusted_parent_place() {
    let error = check_region_program(
        r"
func invalid(value: &Untrusted): void {
    region temp using value {}
    return
}
",
    )
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::InvalidAllocationContext));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0399");
}

#[test]
fn region_handles_cannot_be_moved_or_explicitly_dropped() {
    for (statement, code) in [
        ("let _ = temp", "E0377"),
        ("let _ = move temp", "E0377"),
        ("drop temp", "E0383"),
        ("let closure = (move temp;) { return }", "E0377"),
    ] {
        let error = check_region_program(&format!(
            r"
func invalid(allocator: &+Allocator): void {{
    region temp using allocator {{
        {statement}
    }}
    return
}}
"
        ))
        .unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), code);
    }
}

#[test]
fn an_owned_region_parent_remains_borrowed_until_region_release() {
    let error = check_region_program(
        r"
func invalid(allocator: Allocator): void {
    region temp using allocator {
        drop allocator
    }
    return
}
",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0397");

    check_region_program(
        r"
func valid(allocator: Allocator): void {
    region temp using allocator {}
    drop allocator
    return
}
",
    )
    .unwrap();
}

#[test]
fn current_region_storage_cannot_escape_through_return() {
    let error = check_region_program(
        r"
func invalid(allocator: &+Allocator): Vec<i32> {
    region temp using allocator {
        return Vec [1]
    }
}
",
    )
    .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0395");
}

#[test]
fn nested_return_cleanup_releases_each_region_after_its_values() {
    let output = check_region_program(
        r"
func finish(allocator: Allocator): void {
    region outer using allocator {
        let first = Owned { value: 1 }
        region inner using outer {
            let second = Owned { value: 2 }
            return
        }
    }
}
",
    )
    .unwrap();
    let (body, return_) = output
        .program()
        .bodies()
        .iter()
        .find_map(|(_, body)| {
            let region_count = body
                .nodes()
                .iter()
                .filter(|(_, checked)| {
                    matches!(
                        checked.operation(),
                        CheckedOperation::Control(CheckedControl::Region { .. })
                    )
                })
                .count();
            (region_count == 2).then(|| {
                body.nodes().iter().find_map(|(node, checked)| {
                    matches!(
                        checked.operation(),
                        CheckedOperation::Control(CheckedControl::Return(_))
                    )
                    .then_some((body, node))
                })
            })?
        })
        .unwrap();
    let actions = body
        .cleanups()
        .actions(return_, CleanupTiming::BeforeTransfer)
        .unwrap();

    assert!(matches!(
        actions[0].target(),
        CleanupTarget::Path(path) if matches!(path.root(), PlaceRoot::Local(_))
    ));
    assert!(matches!(actions[1].target(), CleanupTarget::Region { .. }));
    assert!(matches!(
        actions[2].target(),
        CleanupTarget::Path(path) if matches!(path.root(), PlaceRoot::Local(_))
    ));
    assert!(matches!(actions[3].target(), CleanupTarget::Region { .. }));
    assert!(matches!(
        actions.last().unwrap().target(),
        CleanupTarget::Path(path) if matches!(path.root(), PlaceRoot::Parameter(_))
    ));
}

#[test]
fn loop_transfers_release_regions_after_body_values() {
    let output = check_region_program(
        r"
func stop(allocator: &+Allocator): void {
    loop {
        region temp using allocator {
            let value = Owned { value: 1 }
            break
        }
    }
    return
}
func repeat(allocator: &+Allocator): void {
    loop {
        region temp using allocator {
            let value = Owned { value: 1 }
            continue
        }
    }
}
",
    )
    .unwrap();

    for body in output.program().bodies().iter().map(|(_, body)| body) {
        for (node, checked) in body.nodes().iter() {
            if !matches!(
                checked.operation(),
                CheckedOperation::Control(CheckedControl::Break(_) | CheckedControl::Continue(_))
            ) {
                continue;
            }
            let actions = body
                .cleanups()
                .actions(node, CleanupTiming::BeforeTransfer)
                .unwrap();
            assert!(matches!(actions[0].target(), CleanupTarget::Path(_)));
            assert!(matches!(actions[1].target(), CleanupTarget::Region { .. }));
        }
    }
}

#[test]
fn outcome_propagation_releases_a_region_on_only_the_failure_edge() {
    let output = check_region_program(
        r"
func propagate(input: i32?, allocator: &+Allocator): i32? {
    region temp using allocator {
        let value = Owned { value: 1 }
        return input?
    }
}
",
    )
    .unwrap();
    let (body, propagation) = output
        .program()
        .bodies()
        .iter()
        .find_map(|(_, body)| {
            body.nodes().iter().find_map(|(node, checked)| {
                matches!(
                    checked.operation(),
                    CheckedOperation::Outcome(crate::CheckedOutcome::Propagate { .. })
                )
                .then_some((body, node))
            })
        })
        .unwrap();
    let actions = body
        .cleanups()
        .actions(propagation, CleanupTiming::OnOutcomePropagation)
        .unwrap();

    assert!(matches!(actions[0].target(), CleanupTarget::Path(_)));
    assert!(matches!(actions[1].target(), CleanupTarget::Region { .. }));
}

#[test]
fn never_termination_does_not_invent_region_cleanup() {
    let output = check_region_program(
        r"
func stop(): never { loop {} }
func diverge(allocator: &+Allocator): never {
    region temp using allocator {
        let value = Owned { value: 1 }
        stop()
    }
}
",
    )
    .unwrap();
    let (body, region, region_body) = output
        .program()
        .bodies()
        .iter()
        .find_map(|(_, body)| {
            body.nodes().iter().find_map(|(node, checked)| {
                let CheckedOperation::Control(CheckedControl::Region {
                    body: region_body, ..
                }) = checked.operation()
                else {
                    return None;
                };
                Some((body, node, *region_body))
            })
        })
        .unwrap();

    assert!(
        body.cleanups()
            .schedule(region, CleanupTiming::BeforeTransfer)
            .is_none()
    );
    assert!(
        body.cleanups()
            .schedule(region_body, CleanupTiming::BeforeTransfer)
            .is_none()
    );
}
