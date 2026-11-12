//! Whole physical source input uses the same staged header/body/value engine as a single source.
use super::source_scopes::Fixture;
use super::*;
use accepted::{ModuleSourceUnit, SourceFailure};
use nocter_compile_input::ModuleSourceKind::{Implementation, Root};
use nocter_declarations::value_access::PayloadSchema;

fn counts(body_increment: u64) -> Fixture {
    let body_count =
        format!("const BASE: usize = 39\nconst func width(): usize {{ BASE + {body_increment} }}");
    Fixture::new(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./a.nct\nsee ./root_count.nct\nfunc run(): [i32; width()]",
            ),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nsee ./body_count.nct\nfunc run(): [i32; width()] { [0; 42] }",
            ),
            (
                "/app/root_count.nct",
                Implementation,
                "const BASE: usize = 40\nconst func width(): usize { BASE + 2 }",
            ),
            ("/app/body_count.nct", Implementation, &body_count),
        ],
        &[(0, 0, 1), (0, 1, 2), (1, 0, 0), (1, 1, 3)],
    )
}

#[test]
fn module_source_coordinator_executes_each_header_supplier_before_accepting_ordinary_bodies() {
    let fixture = counts(3);
    let source = ModuleSourceUnit::prepare(&fixture.input()).unwrap();
    let program = source.unit().program();
    let run = program
        .resolve(program.symbols().get("run").unwrap())
        .unwrap();
    let result = program.codes().get(run).unwrap().result();
    assert!(
        matches!(program.data_schema(result), Some(PayloadSchema::Array(array)) if array.length() == 42)
    );
    let context = source.declarations().current().unwrap();
    source.unit().bind(&context).unwrap();
    assert_eq!(
        context.for_code(run).unwrap().syntax().tree().source(),
        fixture.tree(1).source()
    );
    assert!(source.unit().program().headers_complete());
}

#[test]
fn module_source_coordinator_rejects_real_count_disagreement_at_both_physical_headers() {
    let fixture = counts(4);
    let Err(SourceFailure::Modules(declarations::ModuleSourcesFailure::Contract(error))) =
        ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("actual header disagreement")
    };
    assert_eq!(
        error.rule(),
        nocter_declaration_lowering::DeclarationContractRule::MismatchedBody
    );
    assert_eq!(error.source().primary().source(), fixture.tree(1).source());
    assert_eq!(
        error.source().notes()[0].origin().source(),
        fixture.tree(0).source()
    );
}

#[test]
fn module_source_coordinator_checks_unused_sources_and_projects_their_actual_rejections() {
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./a.nct\nfunc run(): i32"),
            (
                "/app/a.nct",
                Implementation,
                "see ./index.nct\nfunc run(): i32 { 42 }",
            ),
            (
                "/app/unused.nct",
                Implementation,
                "func unused(): i32 { missing }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let Err(SourceFailure::Checking(accepted::Failure::Body {
        failure: Failure::Rejected(error),
        ..
    })) = ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("unused body")
    };
    assert_eq!(error.rule(), Rule::UnknownName);
    assert_eq!(error.source().primary().source(), fixture.tree(2).source());
}

#[test]
fn module_source_coordinator_keeps_header_cycle_diagnostics_on_complete_representation_fields() {
    let fixture = Fixture::new(
        &[
            ("/app/index.nct", Root, "see ./body.nct\nstruct Node"),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nstruct Node { next: Node }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
    );
    let Err(SourceFailure::Declarations(declarations::Failure::Headers(errors))) =
        ModuleSourceUnit::prepare(&fixture.input())
    else {
        panic!("record header cycle")
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].source().primary().source(),
        fixture.tree(1).source()
    );
    assert!(matches!(
        errors[0].violation().rule,
        nocter_declarations::value_access::DeclarationRule::RecursivePayload { .. }
    ));
}

#[test]
fn module_source_coordinator_accepts_empty_library_sources_and_rejects_unselected_primitives() {
    let empty = Fixture::new(&[("/app/index.nct", Root, "")], &[]);
    let source = ModuleSourceUnit::prepare(&empty.input()).unwrap();
    assert!(source.unit().program().codes().is_empty());
    let primitive = Fixture::new(
        &[
            ("/app/index.nct", Root, ""),
            (
                "/app/body.nct",
                Implementation,
                "primitive func operation(): i32",
            ),
        ],
        &[],
    );
    let Err(SourceFailure::Declarations(declarations::Failure::Primitive(error))) =
        ModuleSourceUnit::prepare(&primitive.input())
    else {
        panic!("unselected primitive")
    };
    assert_eq!(error.rule(), declarations::PrimitiveRule::Unauthorized);
    assert_eq!(
        error.source().primary().source(),
        primitive.tree(1).source()
    );
}
