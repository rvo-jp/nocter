use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableKind, LiteralShape};
use nocter_model::{BuiltinType, TypeKind};

use super::check_prepared_program;
use crate::test_support::Fixture;
use crate::{CheckedOperation, SequenceElement, StaticDispatch, prepare_program_checking};

fn checked(source: &str) -> crate::CheckedProgramOutput {
    let fixture = Fixture::new(source);
    let (input, prelude) = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared).unwrap()
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
            CheckedOperation::Sequence(sequence) => Some((node.ty(), sequence)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sequences.len(), 2);
    assert_eq!(sequences[0].1.elements().len(), 3);
    assert!(
        sequences[0]
            .1
            .elements()
            .iter()
            .all(|element| matches!(element, SequenceElement::Value(_)))
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
