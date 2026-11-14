//! Actual closed source keeps its concretely projected resource contracts through publication.
use super::*;
use nocter_constant_evaluation::state::PlanSource as _;
use nocter_test_support::ValueSourceFixture;

#[test]
fn concrete_resource_contracts_survive_projection_and_invalid_constraints_publish_nothing() {
    let source = ValueSourceFixture::new(
        "const func first(): i32 { 41 }\nconst func run(): i32 { let amount = 1\nlet callback = (amount;): i32 { first() + amount }\nlet next = callback\nnext() }",
    );
    let declarations =
        nocter_declaration_lowering::access_declarations::Query::prepare(source.syntax()).unwrap();
    let context = declarations.bind(source.syntax()).unwrap();
    let queries = context
        .program()
        .codes()
        .iter()
        .map(|(id, _)| {
            (
                id.into(),
                crate::value_access::Query::prepare(&context, id).unwrap(),
            )
        })
        .collect();
    let checked = crate::value_access::accepted::check_queries(&context, queries).unwrap();
    let root = checked
        .program
        .resolve_code(checked.program.symbols().get("run").unwrap())
        .unwrap();
    let mut work = plan::WorkBudget::new(plan::Limits::default().steps());
    let (mut program, mut source) =
        Source::prepare(&checked, root.into(), |_| None, &mut work).unwrap();
    let id = plan::CodeInstance::from(root);
    let extension = source.load(id).unwrap();
    let body = &extension.plans[&id];
    let effects = body
        .points
        .iter()
        .flat_map(|(_, point)| point.actions.iter())
        .flat_map(|action| action.alternatives.iter())
        .flat_map(|alternative| alternative.before.iter().chain(&alternative.after))
        .collect::<Vec<_>>();
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, plan::Effect::MoveRoot { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, plan::Effect::Context { .. }))
    );
    assert!(!effects.iter().any(|effect| matches!(effect, plan::Effect::Move { destination, .. } if destination.fields.is_empty())));
    let mut invalid = plan::ProgramExtension {
        types: extension.types.clone(),
        contracts: extension.contracts.clone(),
        plans: extension.plans.clone(),
        statics: extension.statics.clone(),
    };
    let body = invalid.plans.get_mut(&id).unwrap();
    let mut changed = false;
    body.points = body.points.map(|_, point| {
        let mut point = point.clone();
        for action in &mut point.actions {
            for alternative in &mut action.alternatives {
                for effect in alternative.before.iter_mut().chain(&mut alternative.after) {
                    if let plan::Effect::Context { data, .. } = effect {
                        *data = usize::MAX;
                        changed = true;
                    }
                }
            }
        }
        point
    });
    assert!(changed);
    assert!(matches!(
        program.extend(invalid),
        Err(plan::Rule::InvalidPlan)
    ));
    program.extend(extension).unwrap();
}
