use super::*;
use crate::value_access::storage::prepare_data;
use nocter_checking::value_access::accepted::ModuleSourceUnit;
use nocter_compile_input::ModuleSourceKind::{Implementation, Root};

fn source() -> nocter_test_support::ValueModuleFixture {
    nocter_test_support::ValueModuleFixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                concat!(
                    "see ./body.nct\nconst WIDTH: usize\n",
                    "enum Packet { empty\nwords(data: [i32; WIDTH], flag: bool)\n",
                    "report(data: error) }\ncopy enum Secret",
                ),
            ),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nconst WIDTH: usize = 3\ncopy enum Secret { hidden(value: (void, i32)) }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    )
}

fn schema<'a>(program: &'a Program, name: &str) -> &'a Enumeration {
    program
        .enumeration(
            program
                .resolve_nominal(program.symbols().get(name).unwrap())
                .unwrap(),
        )
        .unwrap()
}

#[test]
fn declared_enum_fields_retain_case_identity_in_target_storage() {
    let fixture = source();
    let input = fixture.input();
    let unit = ModuleSourceUnit::prepare(&input).unwrap();
    let program = unit.unit().program();
    let packet = schema(program, "Packet");
    let secret = schema(program, "Secret");
    let key = program.instantiate_closed_data(packet.data()).unwrap();
    let hidden = program.instantiate_closed_data(secret.data()).unwrap();
    let prepared = prepare_data(
        program,
        RuntimeAbiIdentity::Arm64DarwinV1,
        [key.clone(), hidden.clone()],
    )
    .unwrap();
    let runtime = prepared.environment();
    let packet_id = prepared.instance(program, &key).unwrap();
    let secret_id = prepared.instance(program, &hidden).unwrap();
    assert_ne!(packet_id, secret_id);
    let Some(RuntimeTypeRepresentation::ValueEnum { variants }) =
        runtime.type_representations().get(packet_id)
    else {
        panic!("tagged Packet storage");
    };
    let [empty, words, error] = variants.as_ref() else {
        panic!("exact case count")
    };
    assert_eq!(
        [empty.variant(), words.variant(), error.variant()],
        packet.variants()
    );
    assert!(empty.fields().is_empty());
    let [(array, array_ty), (flag, flag_ty)] = words.fields() else {
        panic!("word fields")
    };
    assert_eq!(
        [*array, *flag],
        program.variants().get(words.variant()).unwrap().fields()
    );
    assert_eq!(
        runtime.types().get(*flag_ty),
        Some(&RuntimeType::Primitive(RuntimePrimitive::Bool))
    );
    let element_ty = runtime
        .value_field(*array_ty, ValueField::Array(2))
        .unwrap();
    assert_eq!(
        runtime.types().get(element_ty),
        Some(&RuntimeType::Primitive(RuntimePrimitive::for_builtin(
            BuiltinType::I32
        )))
    );
    let select = ValueField::EnumField {
        variant: words.variant(),
        field: *array,
    };
    assert_eq!(
        runtime.value_path(
            packet_id,
            &[select, ValueField::Array(2)],
            |&index, length| index < length
        ),
        Some(element_ty)
    );
    assert_eq!(
        runtime.value_field(packet_id, ValueField::Record(*array)),
        None
    );
    for invalid in [
        ValueField::EnumField {
            variant: error.variant(),
            field: *array,
        },
        ValueField::EnumField {
            variant: secret.variants()[0],
            field: *array,
        },
    ] {
        assert!(key.component(program, invalid).is_err());
        assert_eq!(runtime.value_field(packet_id, invalid), None);
    }
}

#[test]
fn closed_enum_storage_rejects_independently_excluded_fields() {
    let fixture = source();
    let input = fixture.input();
    let unit = ModuleSourceUnit::prepare(&input).unwrap();
    let program = unit.unit().program();
    let packet = schema(program, "Packet");
    let excluded = program
        .variant_components(packet.data(), packet.variants()[1])
        .unwrap()
        .next()
        .unwrap()
        .field;
    let fields = packet
        .variants()
        .iter()
        .flat_map(|&case| program.variant_components(packet.data(), case).unwrap())
        .map(|component| {
            let field = if component.field == excluded {
                program.instantiate_uninhabited_data(component.data)
            } else {
                program.instantiate_closed_data(component.data)
            }
            .unwrap();
            (component.field, field)
        });
    assert!(
        program
            .instantiate_components(packet.data(), fields)
            .is_err()
    );
}
