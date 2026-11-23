use super::*;
use nocter_checking::value_access::accepted::ModuleSourceUnit;
use nocter_compile_input::ModuleSourceKind::{Implementation, Root};
use nocter_model::{AccessFieldId, ArenaBuilder, ConstantValue, FrozenData, FrozenNode};
use nocter_runtime_contract::{
    RuntimeTypeRepresentation, RuntimeTypeRepresentationTable, RuntimeValueVariantRepresentation,
};

fn selects_enum_field(place: &Place) -> bool {
    place.projections.iter().any(|projection| {
        matches!(
            projection,
            Projection::Field(nocter_model::ValueField::EnumField { .. })
        )
    })
}

fn lower_module(index: &str, body: &str) -> Program {
    let index = format!("see ./body.nct\n{index}");
    let body = format!("see ./index.nct\n{body}");
    let fixture = nocter_test_support::ValueModuleFixture::new(
        &[
            ("/app/index.nct", Root, &index),
            ("/app/body.nct", Implementation, &body),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let source = ModuleSourceUnit::prepare(&fixture.input())
        .unwrap()
        .into_unit();
    lower_unit(source)
}

fn tagged_program() -> (Program, nocter_model::ValueStatic) {
    let mut program = lower_module(
        "enum State { empty\npresent }\nfunc run(): i32",
        concat!(
            "static DATA: State = State.empty\n",
            "func run(): i32 {\n",
            "let first = if DATA is State.present { 1 } else { 2 }\n",
            "match DATA { State.empty { first }\nState.present { first + 40 } }\n",
            "}",
        ),
    );
    let (&id, data) = program.statics.first_key_value().unwrap();
    let data = data.data;
    let integer = program.bodies[&program.root].signature.result();
    let &[empty, present] = program.runtime.scalar_enum_variants(data).unwrap() else {
        panic!("two cases")
    };
    let field = ArenaBuilder::<AccessFieldId, ()>::new().insert(());
    let mut representations: BTreeMap<_, _> = program
        .runtime
        .type_representations()
        .iter()
        .map(|(data, repr)| (data, repr.clone()))
        .collect();
    representations.insert(
        data,
        RuntimeTypeRepresentation::ValueEnum {
            variants: Box::new([
                RuntimeValueVariantRepresentation::new(empty, []),
                RuntimeValueVariantRepresentation::new(present, [(field, integer)]),
            ]),
        },
    );
    program.runtime = RuntimeEnvironment::new(
        program.runtime.types().clone(),
        RuntimeTypeRepresentationTable::new(representations),
        program.runtime.abi(),
    )
    .unwrap();
    (program, id)
}

#[test]
fn frozen_static_enum_checks_only_the_selected_declared_fields() {
    let (mut program, id) = tagged_program();
    let case = |tag, fields: Box<[usize]>| FrozenNode::Enum { tag, fields };
    let number = FrozenNode::Scalar(ConstantValue::Integer(42));
    let bool_value = FrozenNode::Scalar(ConstantValue::Bool(false));
    for (nodes, valid) in [
        (vec![case(0, Box::new([]))], true),
        (vec![number.clone(), case(1, vec![0].into())], true),
        (vec![bool_value, case(1, vec![0].into())], false),
        (vec![case(1, Box::new([]))], false),
        (vec![number.clone(), case(0, vec![0].into())], false),
        (vec![number, case(2, vec![0].into())], false),
    ] {
        program.statics.get_mut(&id).unwrap().value = FrozenData::new(nodes).unwrap();
        assert_eq!(validate::program(&program).is_ok(), valid);
    }
}

#[test]
fn tagged_enum_branches_keep_their_checked_case_domain() {
    let (program, _) = tagged_program();
    validate::program(&program).unwrap();
    // The source if-is and match use the same operation and their own case tables.
    for arity in [1, 2] {
        let mut dispatch = program.clone();
        let body = dispatch.bodies.get_mut(&dispatch.root).unwrap();
        let cases = body
            .terminators
            .values_mut()
            .find_map(|terminator| match terminator {
                Terminator::EnumSwitch { cases, .. } if cases.len() == arity => Some(cases),
                _ => None,
            })
            .expect("source enum dispatch");
        Arc::make_mut(cases).last_mut().unwrap().0 = 2;
        assert!(validate::program(&dispatch).is_err());
    }
}

#[test]
fn enum_field_addresses_require_the_exact_unmodified_case_edge() {
    let program = lower_module(
        "enum Entry { first(value: i32)\n second(value: i32) }\nfunc run(): i32",
        concat!(
            "func run(): i32 {\n",
            "    let entry = Entry.first(42)\n",
            "    if entry is Entry.first(number) { number } else { 0 }\n",
            "}",
        ),
    );
    validate::program(&program).unwrap();
    assert!(
        operations(&program.bodies[&program.root]).any(|operation| matches!(
            operation,
            Operation::Storage(Effect::BindAddress { source, .. }) if selects_enum_field(source)
        ))
    );
    for corruption in 0..3 {
        let mut program = program.clone();
        let body = program.bodies.get_mut(&program.root).unwrap();
        let switch = body
            .terminators
            .values_mut()
            .find_map(|term| match term {
                Terminator::EnumSwitch {
                    source,
                    cases,
                    fallback,
                } => Some((source, cases, fallback)),
                _ => None,
            })
            .unwrap();
        let (source, cases, fallback) = switch;
        let case = &mut Arc::make_mut(cases)[0];
        match corruption {
            0 => case.0 = 1,
            1 => {
                // Keep the fallback reachable and send an additional, unproven edge to the case.
                let previous = *fallback;
                let target = case.1;
                *body.terminators.get_mut(&previous).unwrap() = Terminator::Jump(target);
            }
            2 => {
                let action = body
                    .actions
                    .get_mut(&Location {
                        point: case.1,
                        action: 0,
                    })
                    .unwrap();
                let mut operations = action.operations.to_vec();
                operations.insert(
                    0,
                    Operation::Copy {
                        source: source.clone(),
                        destination: source.clone(),
                    },
                );
                action.operations = operations.into();
            }
            _ => unreachable!(),
        }
        assert!(
            validate::program(&program).is_err(),
            "invalid case receipt {corruption}"
        );
    }
}

#[test]
fn enum_take_stages_each_copy_before_releasing_its_prepared_carrier() {
    let contract = "copy enum Entry { empty\n pair(first: i32, second: i32) }\nfunc run(): i32";
    for branch in [
        "if take entry^ is Entry.pair(left, right) { left + right } else { 0 }",
        "match take entry { Entry.pair(left, right) { left + right }\n _ { 0 } }",
    ] {
        let program = lower_module(
            contract,
            &format!("func run(): i32 {{\nlet entry = Entry.pair(19, 23)\n{branch}\n}}"),
        );
        validate::program(&program).unwrap();
        let body = &program.bodies[&program.root];
        let (carrier, selected) = body
            .terminators
            .values()
            .find_map(|term| match term {
                Terminator::EnumSwitch { source, cases, .. } => Some((source, cases[0].1)),
                _ => None,
            })
            .unwrap();
        let mut copies = 0;
        let mut retired = false;
        for (_, action) in body.actions.iter().filter(|(at, _)| at.point == selected) {
            let has_disposal = |effects: &[Effect]| {
                effects.iter().any(
                    |effect| matches!(effect, Effect::Dispose { source, .. } if source == carrier),
                )
            };
            retired |= action
                .alternatives
                .iter()
                .any(|path| has_disposal(&path.before));
            for operation in &action.operations {
                if let Operation::Copy { source, .. } = operation
                    && source.slot == carrier.slot
                    && selects_enum_field(source)
                {
                    assert!(!retired);
                    copies += 1;
                }
            }
            retired |= action
                .alternatives
                .iter()
                .any(|path| has_disposal(&path.after));
        }
        assert_eq!(copies, 2);
        assert!(retired);
    }
}
