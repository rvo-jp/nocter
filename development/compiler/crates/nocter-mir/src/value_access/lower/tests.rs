//! A valid frame binding is not a substitute for an original value receipt.
use super::*;
use nocter_checking::value_access::accepted::SourceUnit;
use nocter_runtime_contract::RuntimeAbiIdentity;
use nocter_test_support::ValueSourceFixture;
use std::sync::Arc;

#[test]
fn contextual_aliases_use_actual_inputs_without_a_frame_type_fallback() {
    let fixture = ValueSourceFixture::new(
        "func first(): i32 { 42 }\nfunc forward(action: func(): i32): i32 { read selected = action\nselected() }\nfunc run(): i32 { forward(first) }",
    );
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    let code = |name| {
        unit.program()
            .resolve_code(unit.program().symbols().get(name).unwrap())
            .unwrap()
    };
    let root = code("run");
    let forward = code("forward");
    let instances = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
    let (id, instance) = instances
        .instances()
        .iter()
        .find(|(_, instance)| {
            instance.key().code() == nocter_model::ValueCode::Declaration(forward)
        })
        .unwrap();
    let storage = storage::prepare(&instances, RuntimeAbiIdentity::Arm64DarwinV1, []).unwrap();
    let checked = instances.body(id).unwrap();
    let input = checked.typed().body().inputs().explicit()[0].local;
    let formal = checked.typed().body().locals().get(input).unwrap().data;
    assert!(
        instances
            .unit()
            .program()
            .instantiate_data(formal, instance.key())
            .is_ok()
    );
    let mut builder = Builder {
        runtime: storage.environment(),
        storage: &storage,
        id,
        instances: &instances,
        instance,
        checked,
        at: None,
        slots: ArenaBuilder::new(),
        keys: BTreeMap::new(),
        geometry: BTreeSet::new(),
    };
    assert!(
        builder.closed_data(formal).is_err(),
        "a frame binding cannot certify an occurrence-free shape"
    );
    let actual = builder
        .payload_data(instance.input(input).unwrap())
        .unwrap();
    assert_eq!(
        builder
            .root(Holder::Local(input), actual, true)
            .unwrap()
            .root,
        actual
    );
    // The complete ordinary path must bind and call the alias using its actual definition.
    let lifecycle = lifecycle::Program::prepare(instances).unwrap();
    super::super::Program::lower(&lifecycle, RuntimeAbiIdentity::Arm64DarwinV1).unwrap();
}

#[test]
fn evaluated_callback_referents_require_the_original_commit_root() {
    use nocter_checking::value_access::Referent;
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-callable-arrays.nct"
    )));
    let unit = Arc::new(SourceUnit::prepare(fixture.syntax()).unwrap().into_unit());
    let root = unit
        .program()
        .resolve_code(unit.program().symbols().get("run").unwrap())
        .unwrap();
    let instances = Arc::new(InstanceProgram::prepare(unit, root).unwrap());
    let lifecycle = lifecycle::Program::prepare(instances.clone()).unwrap();
    let storage = storage::prepare(
        &instances,
        RuntimeAbiIdentity::Arm64DarwinV1,
        lifecycle.payload_instances(),
    )
    .unwrap();
    let mut covered = 0;
    for (id, instance) in instances.instances().iter() {
        let checked = instances.body(id).unwrap();
        for (point, block) in checked.graph().points().iter() {
            for (action, operation) in block.actions.iter().enumerate() {
                let Operation::CommitCall(call) = operation.operation else {
                    continue;
                };
                let mut builder = Builder {
                    runtime: storage.environment(),
                    storage: &storage,
                    id,
                    instances: &instances,
                    instance,
                    checked,
                    at: Some(flow::Location { point, action }),
                    slots: ArenaBuilder::new(),
                    keys: BTreeMap::new(),
                    geometry: BTreeSet::new(),
                };
                for (index, argument) in
                    builder.arguments(call).unwrap().to_vec().iter().enumerate()
                {
                    let ArgumentAction::ReserveRead(source @ Referent::Evaluated(_)) =
                        argument.action
                    else {
                        continue;
                    };
                    let data = builder.argument(call, index).unwrap().data;
                    let selected = builder.referent(source, data).unwrap();
                    assert_eq!(selected.root, data);
                    assert!(
                        builder
                            .referent(source, RuntimeDataId::Instance(u32::MAX))
                            .is_err()
                    );
                    builder.at = None;
                    assert!(builder.referent(source, data).is_err());
                    builder.at = Some(flow::Location { point, action });
                    covered += 1;
                }
            }
        }
    }
    assert_eq!(covered, 5);
}
