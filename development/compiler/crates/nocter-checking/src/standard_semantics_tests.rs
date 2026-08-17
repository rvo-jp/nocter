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
fn format_contract_requires_a_public_owned_string_surface() {
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
        PreparationError::StandardSemantics(StandardSemanticError::InvalidFormatContract)
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
