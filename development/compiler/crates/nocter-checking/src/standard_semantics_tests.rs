use nocter_declaration_lowering::{StandardRoleInput, lower_compile_unit_declarations};
use nocter_declarations::StandardDeclarationRole;
use nocter_syntax::NodeKind;

use crate::test_support::Fixture;
use crate::{PreparationError, StandardSemanticError, prepare_program_checking};

fn with_prepared_roles<T>(
    fixture: &Fixture,
    roles: Vec<StandardRoleInput>,
    inspect: impl FnOnce(&crate::PreparedChecking) -> T,
) -> Result<T, PreparationError> {
    let (input, prelude) = fixture.input(false);
    let input = input.with_standard_roles(roles);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index)?;
    Ok(inspect(&prepared))
}

#[test]
fn exact_standard_format_contract_is_accepted() {
    let fixture = Fixture::with_standard(
        "",
        r"
pub struct String {}
pub interface Format {
    pub method &self.format_into(output: &+String): void
}
",
    );
    with_prepared_roles(
        &fixture,
        vec![
            StandardRoleInput::new(
                StandardDeclarationRole::OwnedString,
                fixture.standard_declaration_token(NodeKind::StructDeclaration, "String"),
            ),
            StandardRoleInput::new(
                StandardDeclarationRole::FormatInterface,
                fixture.standard_declaration_token(NodeKind::InterfaceDeclaration, "Format"),
            ),
            StandardRoleInput::new(
                StandardDeclarationRole::FormatMethod,
                fixture.standard_declaration_token(NodeKind::InterfaceMethod, "format_into"),
            ),
        ],
        |prepared| {
            assert!(
                prepared
                    .standard_semantics()
                    .nominal(StandardDeclarationRole::OwnedString)
                    .is_some()
            );
            assert!(
                prepared
                    .standard_semantics()
                    .interface(StandardDeclarationRole::FormatInterface)
                    .is_some()
            );
            assert!(
                prepared
                    .standard_semantics()
                    .callable(StandardDeclarationRole::FormatMethod)
                    .is_some()
            );
        },
    )
    .unwrap();
}

#[test]
fn near_miss_format_contract_is_rejected_once_during_preparation() {
    let fixture = Fixture::with_standard(
        "",
        r"
pub struct String {}
pub interface Format {
    pub method &self.format_into(output: &String): void
}
",
    );
    let error = with_prepared_roles(
        &fixture,
        vec![
            StandardRoleInput::new(
                StandardDeclarationRole::OwnedString,
                fixture.standard_declaration_token(NodeKind::StructDeclaration, "String"),
            ),
            StandardRoleInput::new(
                StandardDeclarationRole::FormatInterface,
                fixture.standard_declaration_token(NodeKind::InterfaceDeclaration, "Format"),
            ),
            StandardRoleInput::new(
                StandardDeclarationRole::FormatMethod,
                fixture.standard_declaration_token(NodeKind::InterfaceMethod, "format_into"),
            ),
        ],
        |_| (),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PreparationError::StandardSemantics(StandardSemanticError::InvalidFormatContract)
    ));
}

#[test]
fn exact_standard_interpolation_contract_is_accepted() {
    let fixture = Fixture::with_standard(
        "",
        r"
pub struct String {}
construct String {
    pub func empty(): Self { return Self {} }
}
instance String {
    pub method &+self.push_str(text: &str): void { return }
}
",
    );

    with_prepared_roles(&fixture, interpolation_roles(&fixture), |prepared| {
        let semantics = prepared.standard_semantics();
        assert!(
            semantics
                .callable(StandardDeclarationRole::InterpolationConstructor)
                .is_some()
        );
        assert!(
            semantics
                .callable(StandardDeclarationRole::InterpolationTextAppender)
                .is_some()
        );
    })
    .unwrap();
}

#[test]
fn interpolation_contract_rejects_a_readonly_output_receiver() {
    let fixture = Fixture::with_standard(
        "",
        r"
pub struct String {}
construct String {
    pub func empty(): Self { return Self {} }
}
instance String {
    pub method &self.push_str(text: &str): void { return }
}
",
    );

    let error = with_prepared_roles(&fixture, interpolation_roles(&fixture), |_| ()).unwrap_err();
    assert!(matches!(
        error,
        PreparationError::StandardSemantics(StandardSemanticError::InvalidInterpolationContract)
    ));
}

#[test]
fn standard_nominal_roles_require_a_public_surface() {
    let fixture = Fixture::with_standard(
        "",
        r"
struct String {}
pub interface Format {
    pub method &self.format_into(output: &+String): void
}
",
    );
    let error = with_prepared_roles(
        &fixture,
        vec![
            StandardRoleInput::new(
                StandardDeclarationRole::OwnedString,
                fixture.standard_declaration_token(NodeKind::StructDeclaration, "String"),
            ),
            StandardRoleInput::new(
                StandardDeclarationRole::FormatInterface,
                fixture.standard_declaration_token(NodeKind::InterfaceDeclaration, "Format"),
            ),
            StandardRoleInput::new(
                StandardDeclarationRole::FormatMethod,
                fixture.standard_declaration_token(NodeKind::InterfaceMethod, "format_into"),
            ),
        ],
        |_| (),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PreparationError::StandardSemantics(StandardSemanticError::InvalidNominalContract(
            StandardDeclarationRole::OwnedString
        ))
    ));
}

#[test]
fn exact_standard_iteration_contracts_are_accepted() {
    let fixture = Fixture::with_standard(
        "",
        r"
pub interface Iterator {
    pub type Item
    pub method &+self.next(): Self.Item?
}
pub interface ExactSizeIterator {
    pub method &self.remaining_len(): usize
}
",
    );
    with_prepared_roles(&fixture, iteration_roles(&fixture), |prepared| {
        let semantics = prepared.standard_semantics();
        assert!(
            semantics
                .interface(StandardDeclarationRole::IteratorInterface)
                .is_some()
        );
        assert!(
            semantics
                .associated_type(StandardDeclarationRole::IteratorItem)
                .is_some()
        );
        assert!(
            semantics
                .callable(StandardDeclarationRole::IteratorNextMethod)
                .is_some()
        );
        assert!(
            semantics
                .interface(StandardDeclarationRole::ExactSizeIteratorInterface)
                .is_some()
        );
        assert!(
            semantics
                .callable(StandardDeclarationRole::ExactSizeIteratorRemainingLenMethod)
                .is_some()
        );
    })
    .unwrap();
}

#[test]
fn near_miss_iteration_contracts_are_rejected_during_preparation() {
    let invalid_next = Fixture::with_standard(
        "",
        r"
pub interface Iterator {
    pub type Item
    pub method &self.next(): Self.Item?
}
pub interface ExactSizeIterator {
    pub method &self.remaining_len(): usize
}
",
    );
    let error =
        with_prepared_roles(&invalid_next, iteration_roles(&invalid_next), |_| ()).unwrap_err();
    assert!(matches!(
        error,
        PreparationError::StandardSemantics(StandardSemanticError::InvalidIteratorContract)
    ));

    let invalid_len = Fixture::with_standard(
        "",
        r"
pub interface Iterator {
    pub type Item
    pub method &+self.next(): Self.Item?
}
pub interface ExactSizeIterator {
    pub method &self.remaining_len(): u32
}
",
    );
    let error =
        with_prepared_roles(&invalid_len, iteration_roles(&invalid_len), |_| ()).unwrap_err();
    assert!(matches!(
        error,
        PreparationError::StandardSemantics(
            StandardSemanticError::InvalidExactSizeIteratorContract
        )
    ));
}

#[test]
fn project_declarations_cannot_acquire_standard_authority() {
    let fixture = Fixture::new("struct Allocator {}\n");
    let error = with_prepared_roles(
        &fixture,
        vec![StandardRoleInput::new(
            StandardDeclarationRole::AbortingAllocator,
            fixture.app_declaration_token(NodeKind::StructDeclaration, "Allocator"),
        )],
        |_| (),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PreparationError::StandardSemantics(StandardSemanticError::OutsideStandardPackage(
            StandardDeclarationRole::AbortingAllocator
        ))
    ));
}

#[test]
fn one_declaration_role_cannot_be_supplied_twice() {
    let fixture = Fixture::with_standard("", "struct Allocator {}\n");
    let token = fixture.standard_declaration_token(NodeKind::StructDeclaration, "Allocator");
    let error = with_prepared_roles(
        &fixture,
        vec![
            StandardRoleInput::new(StandardDeclarationRole::AbortingAllocator, token),
            StandardRoleInput::new(StandardDeclarationRole::AbortingAllocator, token),
        ],
        |_| (),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        PreparationError::StandardSemantics(StandardSemanticError::DuplicateRole(
            StandardDeclarationRole::AbortingAllocator
        ))
    ));
}

fn iteration_roles(fixture: &Fixture) -> Vec<StandardRoleInput> {
    vec![
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
    ]
}

fn interpolation_roles(fixture: &Fixture) -> Vec<StandardRoleInput> {
    vec![
        StandardRoleInput::new(
            StandardDeclarationRole::OwnedString,
            fixture.standard_declaration_token(NodeKind::StructDeclaration, "String"),
        ),
        StandardRoleInput::new(
            StandardDeclarationRole::InterpolationConstructor,
            fixture.standard_declaration_token(NodeKind::ConstructionFunction, "empty"),
        ),
        StandardRoleInput::new(
            StandardDeclarationRole::InterpolationTextAppender,
            fixture.standard_declaration_token(NodeKind::InherentMethod, "push_str"),
        ),
    ]
}
