//! Symbolic result scope follows actual declaration/capture contracts, not equal signatures.
use super::*;
use nocter_model::{ValueCode, ValueInvocation, ValueParameter};

#[test]
fn captured_parameter_scope_rejects_uncaptured_foreign_and_stronger_views() {
    let fixture = nocter_test_support::ValueSourceFixture::new(
        "func owner(take value: (once(i32): i32)?, take sibling: (once(i32): i32)?): void { let saved = (value^;) { drop value }\ndrop saved\ndrop sibling }\nfunc foreign(take value: (once(i32): i32)?): void { drop value }",
    );
    let unit = crate::value_access::accepted::SourceUnit::prepare(fixture.syntax())
        .unwrap()
        .into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let parameter = |name, index: usize| FunctionWitness {
        origin: Origin::Parameter(ValueParameter::Component {
            input: program.codes().get(code(name)).unwrap().parameters()[index].into(),
            ordinal: 0,
        }),
        invocation: ValueInvocation::Take,
    };
    let capture = ValueCode::Closure(*unit.closures().keys().next().unwrap());
    let own = parameter("owner", 0);
    assert!(parameter_in_scope(program, capture, own));
    assert!(!parameter_in_scope(program, capture, parameter("owner", 1)));
    assert!(!parameter_in_scope(
        program,
        capture,
        parameter("foreign", 0)
    ));
    assert!(!parameter_in_scope(
        program,
        capture,
        FunctionWitness {
            invocation: ValueInvocation::Read,
            ..own
        }
    ));
    assert!(parameter_in_scope(program, code("owner").into(), own));
    assert!(!parameter_in_scope(
        program,
        code("owner").into(),
        FunctionWitness {
            invocation: ValueInvocation::Read,
            ..own
        }
    ));
    assert!(!parameter_in_scope(program, code("foreign").into(), own));
}
