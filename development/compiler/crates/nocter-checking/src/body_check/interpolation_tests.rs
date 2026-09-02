use crate::test_support::StandardRoleInput;
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::{TypeKind, TypeStore};
use nocter_syntax::NodeKind;
use nocter_toolchain_contract::StandardDeclarationRole;

use super::check_prepared_program;
use crate::test_support::{Fixture, with_standard_roles};
use crate::{
    AllocationSelection, BodyCheckError, BodyRule, CheckedOperation, CleanupTarget, CleanupTiming,
    InterpolationPart, ReadonlyOperandPreparation, StaticDispatch, prepare_program_checking,
};

fn check(standard: &str) -> Result<crate::CheckedProgramOutput, BodyCheckError> {
    let fixture = Fixture::with_standard("", standard);
    let roles = vec![
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
        StandardRoleInput::new(
            StandardDeclarationRole::FormatInterface,
            fixture.standard_declaration_token(NodeKind::InterfaceDeclaration, "Format"),
        ),
        StandardRoleInput::new(
            StandardDeclarationRole::FormatMethod,
            fixture.standard_declaration_token(NodeKind::InterfaceMethod, "format_into"),
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

fn standard_prelude(extra: &str) -> String {
    format!(
        r"
pub struct String {{}}
construct String {{
    pub func empty(): Self {{ return Self {{}} }}
}}
instance String {{
    pub method &+self.push_str(text: &str): void {{ return }}
}}
pub interface Format {{
    pub method &self.try_format_into(output: &+String): void!
    pub default method &self.format_into(output: &+String): void {{
        self.try_format_into(output) catch _ {{ return }}
        return
    }}
}}
{extra}
"
    )
}

fn nominal_definition(types: &TypeStore, ty: nocter_model::TypeId) -> nocter_model::NominalTypeId {
    let Some(TypeKind::Nominal { definition, .. }) = types.get(ty) else {
        panic!("expected a nominal type")
    };
    *definition
}

#[test]
fn interpolation_freezes_source_order_format_dispatch_and_owned_output() {
    let output = check(&standard_prelude(
        r#"
instance i32 {
    impl Format
    method &self.try_format_into(output: &+String): void! { return }
}
func render(value: i32): String {
    "before ${value} after"
}
"#,
    ))
    .unwrap();
    let program = output.program();
    let interpolation = program
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Interpolation(interpolation) => Some((node.ty(), interpolation)),
            _ => None,
        })
        .expect("checked interpolation");

    assert_eq!(interpolation.0, interpolation.1.output());
    assert!(matches!(
        interpolation.1.constructor().dispatch(),
        StaticDispatch::Direct(_)
    ));
    assert!(matches!(
        interpolation.1.text_appender().dispatch(),
        StaticDispatch::Direct(_)
    ));
    assert_eq!(
        nominal_definition(program.types(), interpolation.1.output()),
        program
            .standard_semantics()
            .nominal(StandardDeclarationRole::OwnedString)
            .unwrap()
    );
    assert_eq!(
        interpolation.1.allocation(),
        AllocationSelection::CurrentRegion
    );
    let [
        InterpolationPart::Text(before),
        InterpolationPart::Formatted { operand, formatter },
        InterpolationPart::Text(after),
    ] = interpolation.1.parts()
    else {
        panic!("interpolation parts must preserve authored order")
    };
    assert_eq!(before.as_ref(), "before ");
    assert_eq!(after.as_ref(), " after");
    assert_eq!(
        operand.preparation(),
        ReadonlyOperandPreparation::BorrowPlace
    );
    assert!(matches!(
        formatter.dispatch(),
        StaticDispatch::InterfaceDefault { method, .. }
            if method
                == output
                    .program()
                    .standard_semantics()
                    .callable(StandardDeclarationRole::FormatMethod)
                    .unwrap()
    ));
}

#[test]
fn generic_interpolation_uses_the_exact_format_requirement() {
    let output = check(&standard_prelude(
        r#"
func render<T>(value: &T): String where T impl Format {
    "${value}"
}
"#,
    ))
    .unwrap();
    let formatter = output
        .program()
        .bodies()
        .iter()
        .flat_map(|(_, body)| body.nodes().iter())
        .find_map(|(_, node)| match node.operation() {
            CheckedOperation::Interpolation(interpolation) => {
                interpolation.parts().iter().find_map(|part| match part {
                    InterpolationPart::Formatted { formatter, .. } => Some(formatter),
                    InterpolationPart::Text(_) | InterpolationPart::Diverging(_) => None,
                })
            }
            _ => None,
        })
        .expect("generic formatter selection");

    assert!(matches!(
        formatter.dispatch(),
        StaticDispatch::InterfaceMethod { method, .. }
            if method
                == output
                    .program()
                    .standard_semantics()
                    .callable(StandardDeclarationRole::FormatMethod)
                    .unwrap()
    ));
}

#[test]
fn interpolation_rejects_values_without_the_exact_format_interface_implementation() {
    let error = check(&standard_prelude(
        r#"
struct Value {}
func render(value: &Value): String {
    "${value}"
}
"#,
    ))
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::InvalidInterpolation));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0400");
}

#[test]
fn interpolation_borrows_move_only_places_and_drops_temporary_operands_at_statement_end() {
    let output = check(&standard_prelude(
        r#"
struct Value {}
instance Value {
    impl Format
    method &self.try_format_into(output: &+String): void! { return }
}
func make(): Value { Value {} }
func render(value: Value): void {
    let first = "${value}"
    drop first
    let second = "${make()}"
    drop second
    drop value
    return
}
"#,
    ))
    .unwrap();
    let program = output.program();
    let interpolations = program
        .bodies()
        .iter()
        .flat_map(|(_, body)| {
            body.nodes().iter().filter_map(move |(node, checked)| {
                let CheckedOperation::Interpolation(interpolation) = checked.operation() else {
                    return None;
                };
                Some((body, node, interpolation))
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(interpolations.len(), 2);
    let preparations = interpolations
        .iter()
        .map(|(_, _, interpolation)| {
            interpolation
                .parts()
                .iter()
                .find_map(|part| match part {
                    InterpolationPart::Formatted { operand, .. } => Some(operand.preparation()),
                    InterpolationPart::Text(_) | InterpolationPart::Diverging(_) => None,
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        preparations,
        vec![
            ReadonlyOperandPreparation::BorrowPlace,
            ReadonlyOperandPreparation::BorrowTemporary
        ]
    );
    let temporary_operand = interpolations[1]
        .2
        .parts()
        .iter()
        .find_map(|part| match part {
            InterpolationPart::Formatted { operand, .. } => Some(operand.value()),
            InterpolationPart::Text(_) | InterpolationPart::Diverging(_) => None,
        })
        .unwrap();
    assert!(interpolations[1].0.nodes().iter().any(|(node, _)| {
        interpolations[1]
            .0
            .cleanups()
            .actions(node, CleanupTiming::AtStatementEnd)
            .is_some_and(|actions| {
                actions.iter().any(|action| {
                    matches!(action.target(), CleanupTarget::Value { node, .. } if *node == temporary_operand)
                })
            })
    }));
}

#[test]
fn failed_interpolation_operand_cleans_the_partial_output() {
    let output = check(&standard_prelude(
        r#"
drop String(&+self) { return }
struct Value {}
instance Value {
    impl Format
    method &self.try_format_into(output: &+String): void! { return }
}
func render(input: Value?): String? {
    "prefix ${move input?}"
}
"#,
    ))
    .unwrap();
    let program = output.program();
    let string = program
        .standard_semantics()
        .nominal(StandardDeclarationRole::OwnedString)
        .unwrap();
    let string_type = program
        .types()
        .iter()
        .find_map(|(ty, kind)| {
            matches!(kind, TypeKind::Nominal { definition, arguments } if *definition == string && arguments.is_empty())
                .then_some(ty)
        })
        .unwrap();
    let (_, body, interpolation_node) = program
        .bodies()
        .iter()
        .find_map(|(body_id, body)| {
            body.nodes().iter().find_map(|(node, checked)| {
                matches!(checked.operation(), CheckedOperation::Interpolation(_))
                    .then_some((body_id, body, node))
            })
        })
        .expect("interpolation body");
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
        .expect("propagating operand");
    let actions = body
        .cleanups()
        .actions(propagation, CleanupTiming::OnOutcomePropagation)
        .expect("partial interpolation cleanup on failure");

    assert!(actions.iter().any(|action| {
        matches!(action.target(), CleanupTarget::Value { node, ty }
            if *node == interpolation_node && *ty == string_type)
    }));
}
