use super::*;
use nocter_model::ValueParameter;

#[test]
fn anonymous_instances_keep_call_site_bindings_and_reject_wrong_owners_or_positions() {
    let fixture = ValueSourceFixture::new(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/anonymous-callable-inputs.nct"
    )));
    let unit = accepted(&fixture);
    let graph = InstanceProgram::prepare(unit.clone(), callable(&unit, "run")).unwrap();
    let instances = graph
        .instances()
        .iter()
        .filter_map(|(_, instance)| {
            (!instance.key().witnesses().is_empty()
                && matches!(instance.key().code(), ValueCode::Closure(_)))
            .then_some(instance.key())
        })
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 9);
    assert!(instances.iter().any(|first| {
        instances
            .iter()
            .any(|second| first.code() == second.code() && first.witnesses() != second.witnesses())
    }));
    let selected = instances[0];
    let (&formal, &actual) = selected.witnesses().first_key_value().unwrap();
    let ValueParameter::Closure { owner, position } = formal else {
        panic!("anonymous input")
    };
    assert_eq!(selected.code(), ValueCode::Closure(owner));
    let wrong = ValueParameter::Closure {
        owner,
        position: position + 1,
    };
    assert!(unit.program().bind_code(selected.code(), []).is_err());
    assert!(
        unit.program()
            .bind_code(selected.code(), [(wrong, actual.witness())])
            .is_err()
    );
    let another = instances
        .iter()
        .find(|key| key.code() != selected.code())
        .unwrap();
    assert!(
        unit.program()
            .bind_code(
                selected.code(),
                another
                    .witnesses()
                    .iter()
                    .map(|(&parameter, witness)| (parameter, witness.witness()))
            )
            .is_err()
    );
}
