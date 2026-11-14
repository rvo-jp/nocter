//! Original cleanup roles, independently of source admission of contextual payloads.
use super::*;
use crate::value_access::flow::resources::{self, Holder};
use nocter_model::{ArenaBuilder, BuiltinType, ValueFunctionOrigin, ValueInvocation};
use nocter_test_support::ValueSourceFixture;

#[test]
fn scoped_cleanup_conditions_retain_the_original_retired_step_and_reject_invalid_cursors() {
    let source = ValueSourceFixture::new(
        "func replace(edit target: error, take next: error): void { target = next^ }",
    );
    let unit = crate::value_access::accepted::SourceUnit::prepare(source.syntax())
        .unwrap()
        .into_unit();
    let code = unit
        .program()
        .resolve_code(unit.program().symbols().get("replace").unwrap())
        .unwrap();
    let body = unit.code_body(code.into()).unwrap();
    let mut domain = evaluation::Types::default();
    let conditions = collect(body, &mut domain).unwrap();
    let mut replacements = 0;
    for condition in conditions {
        let original = condition.original(body).unwrap();
        assert_eq!(
            condition.data(body).unwrap(),
            payload(original).unwrap().data()
        );
        if let Step::ReplaceReferent {
            retired,
            payload: supplied,
            ..
        } = original
        {
            assert_eq!(retired.data(), supplied.data());
            assert!(std::ptr::eq(payload(original).unwrap(), retired));
            assert!(!std::ptr::eq(payload(original).unwrap(), supplied));
            let Site::Cleanup(at) = condition.violation().site else {
                panic!("cleanup")
            };
            assert!(matches!(
                body.graph().points().get(at.point).unwrap().actions[at.action].operation,
                flow::Operation::Assign { .. }
            ));
            replacements += 1;
        }
        let mut invalid = condition.clone();
        invalid.step = usize::MAX;
        assert_eq!(invalid.data(body), Err(INVALID));
        let mut invalid = condition.clone();
        invalid.transition = usize::MAX;
        assert_eq!(invalid.data(body), Err(INVALID));
        let mut invalid = condition;
        invalid.violation.rule =
            Rule::UnsupportedData(unit.program().data_type(BuiltinType::Error).unwrap());
        assert_eq!(invalid.data(body), Err(INVALID));
    }
    assert_eq!(replacements, 1);
}

#[test]
fn projected_cleanup_selects_the_retired_context_when_receiving_the_same_source_type() {
    use nocter_declarations::value_access::FunctionWitness;
    let source = ValueSourceFixture::new(
        "func first(): void {}\nfunc second(): void {}\nfunc root(take action: func(): void): void { drop action }",
    );
    let declarations =
        nocter_declaration_lowering::access_declarations::Query::prepare(source.syntax()).unwrap();
    let context = declarations.bind(source.syntax()).unwrap();
    let program = context.program();
    let code = |name| {
        program
            .resolve_code(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let parameter = program.codes().get(code("root")).unwrap().parameters()[0];
    let data = program
        .function_data(FunctionWitness {
            origin: ValueFunctionOrigin::Parameter(parameter.into()),
            invocation: ValueInvocation::Read,
        })
        .unwrap();
    let keys = ["first", "second"].map(|name| {
        program
            .instantiate_function(data, &program.concrete_witness(code(name).into()).unwrap())
            .unwrap()
    });
    assert_eq!(keys[0].data(), keys[1].data());
    assert_ne!(keys[0], keys[1]);
    let contract = resources::payload(program, data).unwrap();
    let [retired, supplied] = keys
        .each_ref()
        .map(|key| contract.instantiate(program, key).unwrap());
    let mut expressions = ArenaBuilder::new();
    let step = Step::ReplaceReferent {
        destination: Holder::Return.place(),
        source: Holder::Expression(expressions.insert(())).place(),
        retired: retired.clone(),
        payload: supplied.clone(),
    };
    assert_eq!(payload(&step).unwrap().instance(), retired.instance());
    assert_ne!(payload(&step).unwrap().instance(), supplied.instance());
}
