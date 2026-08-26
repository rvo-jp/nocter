use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::TypeKind;

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{BodyRule, CallTarget, CheckedOperation, StaticDispatch, prepare_program_checking};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

const SHOW: &str = "
pub interface Show { pub method &self.show(): i32 }
struct Value {}
instance Value {
    impl Show
    method &self.show(): i32 { 1 } }
";

#[test]
fn opaque_result_freezes_one_checked_witness_conversion() {
    let output = check(&format!(
        "{SHOW}\nfunc make(): some Show {{ Value {{}} }}\nfunc read(): i32 {{ make().show() }}\n"
    ))
    .unwrap();
    let (definition, _) = output
        .program()
        .graph()
        .declarations()
        .opaque_types()
        .iter()
        .next()
        .unwrap();
    let witness = output.program().opaque_witnesses().get(definition).unwrap();
    assert!(matches!(
        output.program().types().get(witness),
        Some(TypeKind::Nominal { definition, .. })
            if output
                .program()
                .graph()
                .declarations()
                .nominal_types()
                .get(*definition)
                .and_then(|nominal| output.program().graph().symbols().spelling(nominal.name()))
                == Some("Value")
    ));
    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::OpaqueWitness(conversion)
                    if conversion.definition() == definition && conversion.witness() == witness
            )
        })
    }));
    assert!(output.program().bodies().iter().any(|(_, body)| {
        body.nodes().iter().any(|(_, node)| {
            matches!(
                node.operation(),
                CheckedOperation::Call(call)
                    if matches!(
                        call.target(),
                        CallTarget::Static(selection)
                            if matches!(selection.dispatch(), StaticDispatch::OpaqueMethod { opaque, .. }
                                if matches!(output.program().types().get(opaque), Some(TypeKind::Opaque { definition: actual, .. }) if *actual == definition))
                    )
            )
        })
    }));
}

#[test]
fn optional_opaque_result_erases_the_success_payload_after_injection() {
    let output = check(&format!(
        "{SHOW}\nfunc make(flag: bool): some Show? {{\n    if flag {{ Value {{}} }} else {{ none }}\n}}\n"
    ))
    .unwrap();
    let opaque = output
        .program()
        .graph()
        .declarations()
        .opaque_types()
        .iter()
        .next()
        .unwrap()
        .0;
    assert!(output.program().opaque_witnesses().get(opaque).is_some());
}

#[test]
fn reachable_opaque_returns_must_select_one_witness() {
    let error = check(
        "pub interface Show { pub method &self.show(): i32 }\n\
         struct First {}\n\
         struct Second {}\n\
         instance First {
             impl Show
             method &self.show(): i32 { 1 } }\n\
         instance Second {
             impl Show
             method &self.show(): i32 { 2 } }\n\
         func make(flag: bool): some Show {\n\
             if flag { First {} } else { Second {} }\n\
         }\n",
    )
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::InvalidOpaqueWitness));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0408");
}

#[test]
fn opaque_witness_must_conform_to_the_advertised_interface() {
    let error = check(
        "pub interface Show { pub method &self.show(): i32 }\n\
         struct Value {}\n\
         func make(): some Show { Value {} }\n",
    )
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::InvalidOpaqueWitness));
}

#[test]
fn opaque_associated_bindings_match_the_selected_interface_implementation() {
    let accepted = check(
        "pub interface Source { pub type Item }\n\
         struct Buffer {}\n\
         instance Buffer { impl Source { .Item = i32 } }\n\
         func make(): some Source { .Item = i32 } { Buffer {} }\n",
    );
    assert!(accepted.is_ok());

    let rejected = check(
        "pub interface Source { pub type Item }\n\
         struct Buffer {}\n\
         instance Buffer { impl Source { .Item = i32 } }\n\
         func make(): some Source { .Item = i64 } { Buffer {} }\n",
    )
    .unwrap_err();
    assert_eq!(rejected.rule(), Some(BodyRule::InvalidOpaqueWitness));
}

#[test]
fn generic_opaque_witness_uses_lexical_interface_and_associated_evidence() {
    check(
        "pub interface Source { pub type Item }\n\
         func hide<S>(value: S): some Source { .Item = S.Item } where S impl Source {\n\
             move value\n\
         }\n",
    )
    .unwrap();
}
