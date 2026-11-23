use super::*;
use nocter_checking::value_access::accepted::ModuleSourceUnit;
use nocter_compile_input::ModuleSourceKind::{Implementation, Root};
use nocter_model::{AccessFieldId, ArenaBuilder, ConstantValue, FrozenData, FrozenNode};
use nocter_runtime_contract::{
    RuntimeTypeRepresentation, RuntimeTypeRepresentationTable, RuntimeValueVariantRepresentation,
};

#[test]
fn frozen_static_enum_checks_only_the_selected_declared_fields() {
    let fixture = nocter_test_support::ValueModuleFixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nenum State { empty\npresent }\nfunc run(): i32",
            ),
            (
                "/app/body.nct",
                Implementation,
                concat!(
                    "see ./index.nct\nstatic DATA: State = State.empty\n",
                    "func run(): i32 { read state = DATA\n42 }",
                ),
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let source = ModuleSourceUnit::prepare(&fixture.input())
        .unwrap()
        .into_unit();
    let mut program = lower_unit(source);
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
