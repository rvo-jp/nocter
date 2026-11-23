use super::*;
use nocter_checking::value_access::accepted::ModuleSourceUnit;
use nocter_compile_input::ModuleSourceKind::{Implementation, Root};
use nocter_model::ValueCode;
use nocter_test_support::ValueModuleFixture;

#[test]
fn only_reached_retirements_extend_the_original_instance_graph() {
    let fixture = ValueModuleFixture::new(
        &[
            ("/app/index.nct", Root, "see ./body.nct\nfunc run(): i32"),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\n\
                 struct Live {}\n\
                 struct Nested {}\n\
                 struct Unused {}\n\
                 struct Carrier { child: Live }\n\
                 impl Live { drop { let next = Nested {}\ndrop next } }\n\
                 impl Nested { drop { cleanup_helper() } }\n\
                 impl Unused { drop { unused_helper() } }\n\
                 func cleanup_helper(): void {}\n\
                 func unused_helper(): void {}\n\
                 func run(): i32 { let value = Carrier { child: Live {} }\ndrop value\n42 }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let unit = Arc::new(
        ModuleSourceUnit::prepare(&fixture.input())
            .unwrap()
            .into_unit(),
    );
    let symbol = |name| unit.program().symbols().get(name).unwrap();
    let code = |name| {
        unit.program()
            .codes()
            .iter()
            .find_map(|(id, code)| (code.name() == Some(symbol(name))).then_some(id))
            .unwrap()
    };
    assert_eq!(unit.destructor_frames().count(), 3);
    let instances = Arc::new(InstanceProgram::prepare(Arc::clone(&unit), code("run")).unwrap());
    assert_eq!(instances.instances().len(), 1);

    let frozen = super::super::Program::prepare(Arc::clone(&instances)).unwrap();
    let extended = frozen.instance_program();
    assert_eq!(extended.root(), instances.root());
    assert!(Arc::ptr_eq(&extended.unit, &unit));
    assert_eq!(instances.instances().len(), 1);
    assert_eq!(extended.destructors().count(), 2);
    assert_eq!(frozen.bodies().len(), 4);
    let called = |name| {
        extended
            .instances()
            .iter()
            .any(|(_, instance)| instance.key().code() == ValueCode::Declaration(code(name)))
    };
    assert!(called("cleanup_helper"));
    assert!(!called("unused_helper"));
}
