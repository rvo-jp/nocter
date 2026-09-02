use crate::test_support::StandardRoleInput;
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_syntax::NodeKind;
use nocter_toolchain_contract::StandardDeclarationRole;

use crate::test_support::{Fixture, with_standard_roles};
use crate::{PreparationError, StandardSemanticError, prepare_program_checking};

fn with_prepared_roles<T>(
    fixture: &Fixture,
    roles: Vec<StandardRoleInput>,
    inspect: impl FnOnce(&crate::PreparedChecking) -> T,
) -> Result<T, PreparationError> {
    let input = fixture.input(false);
    let input = with_standard_roles(input, roles);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared = prepare_program_checking(&input, program, &frontend_bindings, source_index)?;
    Ok(inspect(&prepared))
}

#[test]
fn exact_standard_format_contract_is_accepted() {
    let fixture = Fixture::with_standard(
        "",
        r"
pub struct String {}
pub interface Format {
    pub method &self.try_format_into(output: &+String): void!
    pub default method &self.format_into(output: &+String): void { return }
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
    pub method &self.try_format_into(output: &+String): void!
    pub default method &self.format_into(output: &String): void { return }
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
fn bodyless_format_method_is_rejected_once_during_preparation() {
    let fixture = Fixture::with_standard(
        "",
        r"
pub struct String {}
pub interface Format {
    pub method &self.try_format_into(output: &+String): void!
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
    pub method &self.try_format_into(output: &+String): void!
    pub default method &self.format_into(output: &+String): void { return }
}
",
    );
    let input = with_standard_roles(
        fixture.input(false),
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
    );
    let error = lower_compile_unit_declarations(&input).unwrap_err();

    assert!(matches!(
        error,
        nocter_declaration_lowering::DeclarationLoweringError::Toolchain(
            nocter_declaration_lowering::ToolchainError::MissingStandardDeclaration(
                StandardDeclarationRole::OwnedString
            )
        )
    ));
}

#[test]
fn allocation_nominal_roles_require_the_two_word_context_header() {
    let valid = Fixture::with_standard(
        "",
        "pub struct Allocator { first: usize\n    second: usize }\n",
    );
    with_prepared_roles(
        &valid,
        vec![StandardRoleInput::new(
            StandardDeclarationRole::AbortingAllocator,
            valid.standard_declaration_token(NodeKind::StructDeclaration, "Allocator"),
        )],
        |_| (),
    )
    .unwrap();

    let invalid = Fixture::with_standard("", "pub struct AllocationContext {}\n");
    let error = with_prepared_roles(
        &invalid,
        vec![StandardRoleInput::new(
            StandardDeclarationRole::AllocationContext,
            invalid.standard_declaration_token(NodeKind::StructDeclaration, "AllocationContext"),
        )],
        |_| (),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        PreparationError::StandardSemantics(StandardSemanticError::InvalidNominalContract(
            StandardDeclarationRole::AllocationContext
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
fn project_declarations_cannot_acquire_standard_authority_during_lowering() {
    let fixture = Fixture::new("struct Allocator {}\n");
    let input = with_standard_roles(
        fixture.input(false),
        vec![StandardRoleInput::new(
            StandardDeclarationRole::AbortingAllocator,
            fixture.app_declaration_token(NodeKind::StructDeclaration, "Allocator"),
        )],
    );
    let error = lower_compile_unit_declarations(&input).unwrap_err();

    assert!(matches!(
        error,
        nocter_declaration_lowering::DeclarationLoweringError::Toolchain(
            nocter_declaration_lowering::ToolchainError::DeclarationModuleOutsideStandardPackage(_)
        )
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
