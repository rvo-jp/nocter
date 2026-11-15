//! Authored array contexts retain real callable identities and inactive alternatives.
use super::*;

#[test]
fn construction_context_does_not_replace_an_exact_parameter_witness() {
    let fixture = nocter_test_support::ValueSourceFixture::new(
        "func live(value: i32): i32 { value }\nfunc keep(take action: func(i32): i32): void { var saved = action^\nsaved = live }",
    );
    let Err(SourceFailure::Checking(Failure::Body {
        failure: crate::value_access::Failure::Rejected(diagnostic),
        ..
    })) = SourceUnit::prepare(fixture.syntax())
    else {
        panic!("an exact local witness cannot be replaced by signature matching")
    };
    assert_eq!(diagnostic.rule(), crate::value_access::Rule::TypeMismatch);
}

#[test]
fn contextual_absent_elements_keep_their_own_supplied_environment_context() {
    use crate::value_access::ExpressionKind;
    use nocter_declarations::value_access::InstanceSchema;
    use nocter_model::{ValueApplication, ValueField, ValueFunctionOrigin as Origin};
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-absent-callable-arrays.nct"
    ));
    let unit = accepted(
        &source
            .replace("const ANSWER: i32 = calculate()", "")
            .replace("const func", "func"),
    );
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let root = program.bind_code(code("calculate").into(), []).unwrap();
    let mut contexts = Vec::new();
    for (call, expression) in named(&unit, "calculate")
        .typed()
        .body()
        .expressions()
        .iter()
    {
        if !matches!(expression.kind, ExpressionKind::Call { target: Origin::Declaration(target), .. } if target == code("choose"))
        {
            continue;
        }
        let mut work = nocter_constant_evaluation::state::WorkBudget::new(
            std::num::NonZeroU64::new(100_000).unwrap(),
        );
        contexts.push(
            unit.instantiate_invocation(
                &root,
                ValueApplication {
                    owner: root.code(),
                    call,
                },
                &mut work,
            )
            .unwrap(),
        );
    }
    assert_eq!(contexts.len(), 6);
    for pair in contexts.chunks_exact(2) {
        assert_eq!(pair[0], pair[1]);
    }
    assert_ne!(contexts[0], contexts[2]);
    let function = |index: usize| {
        let key = contexts[index]
            .input_data(0)
            .unwrap()
            .component(program, ValueField::Array(0))
            .unwrap()
            .component(program, ValueField::Variant(true))
            .unwrap();
        let InstanceSchema::Function(witness) = key.schema(program).unwrap() else {
            panic!("expected the actual supplied callable")
        };
        witness.clone()
    };
    let [plain, captured] = [0, 2].map(function);
    assert_eq!(plain.witness(), captured.witness());
    assert_ne!(plain, captured);
}

#[test]
fn contextual_absence_in_callable_arrays_uses_the_supplied_witness() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-absent-callable-arrays.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Datum::integer(42));
}

#[test]
fn contextual_callable_arrays_keep_authored_context_and_actual_witness_requirements() {
    for body in [
        "let values = [none, live]\ndrop values",
        "let values = [live, none]\ndrop values",
        "discard([live, other])",
        "discard([none, wrong])",
        "discard([none, if true { live } else { wrong }])",
        "discard([if true { wrong } else { none }, live])",
        "let values = [if true { none } else { live }, live]\ndrop values",
    ] {
        let source = format!(
            "func live(value: i32): i32 {{ value }}\nfunc other(value: i32): i32 {{ value + 1 }}\nfunc wrong(value: u64): u64 {{ value }}\nfunc discard(take values: [(func(i32): i32)?; 2]): void {{ drop values }}\nfunc run(): void {{ {body} }}"
        );
        let fixture = nocter_test_support::ValueSourceFixture::new(&source);
        let Err(SourceFailure::Checking(Failure::Body {
            failure: crate::value_access::Failure::Rejected(diagnostic),
            ..
        })) = SourceUnit::prepare(fixture.syntax())
        else {
            panic!("array context cannot invent or merge callable identities: {body}")
        };
        assert_eq!(diagnostic.rule(), crate::value_access::Rule::TypeMismatch);
    }
}
