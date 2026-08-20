use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::{BuiltinType, FieldId, ParameterId, TypeId, TypeKind};

use super::{ConcreteDestructionKind, ConcreteDispatchResolver};
use crate::test_support::Fixture;
use crate::{
    CheckedProgram, CleanupTarget, TypeSubstitution, check_prepared_program,
    prepare_program_checking,
};

fn check(source: &str) -> crate::CheckedProgramOutput {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    check_prepared_program(&input, prepared).unwrap()
}

fn nominal_named(program: &CheckedProgram, expected: &str, arguments: &[TypeId]) -> TypeId {
    program
        .types()
        .iter()
        .find_map(|(ty, kind)| {
            let TypeKind::Nominal {
                definition,
                arguments: actual,
            } = kind
            else {
                return None;
            };
            let declaration = program
                .graph()
                .declarations()
                .nominal_types()
                .get(*definition)?;
            (program.graph().symbols().spelling(declaration.name()) == Some(expected)
                && actual.as_ref() == arguments)
                .then_some(ty)
        })
        .unwrap_or_else(|| panic!("missing fixture type {expected}"))
}

fn field_name(program: &CheckedProgram, field: FieldId) -> &str {
    program
        .graph()
        .declarations()
        .fields()
        .get(field)
        .and_then(|field| program.graph().symbols().spelling(field.name()))
        .unwrap()
}

fn parameter_name(program: &CheckedProgram, parameter: ParameterId) -> &str {
    program
        .graph()
        .declarations()
        .parameters()
        .get(parameter)
        .and_then(|parameter| program.graph().symbols().spelling(parameter.name()))
        .unwrap()
}

#[test]
fn nominal_glue_preserves_drop_arguments_and_reverse_field_order() {
    let output = check(
        "struct Leaf<T> { value: T }\n\
         drop Leaf<T>(&+self) { return }\n\
         struct Pair<T> {\n\
             first: Leaf<T>\n\
             second: Leaf<T>\n\
         }\n\
         drop Pair<T>(&+self) { return }\n\
         func hold(value: Pair<i32>): void { return }\n",
    );
    let program = output.program();
    let i32_ = program.types().builtin(BuiltinType::I32);
    let pair = nominal_named(program, "Pair", &[i32_]);
    let mut resolver = ConcreteDispatchResolver::new(program);
    let plan = resolver
        .resolve_destruction(pair, &TypeSubstitution::default())
        .unwrap()
        .unwrap();
    let ConcreteDestructionKind::Struct { drop, fields } = plan.kind() else {
        panic!("Pair<i32> must use struct destruction")
    };

    let drop = drop.as_ref().expect("Pair owns a drop body");
    assert_eq!(drop.generic_arguments().as_slice()[0].ty(), i32_);
    assert_eq!(
        fields
            .iter()
            .map(|field| field_name(program, field.field()))
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    for field in fields {
        let ConcreteDestructionKind::Struct { drop, fields } = field.plan().kind() else {
            panic!("Pair fields must recurse into Leaf destruction")
        };
        assert!(fields.is_empty());
        assert_eq!(
            drop.as_ref().unwrap().generic_arguments().as_slice()[0].ty(),
            i32_
        );
    }
}

#[test]
fn enum_glue_retains_only_active_payload_work_in_reverse_order() {
    let output = check(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         enum Choice {\n\
             empty\n\
             pair(first: Owned, second: Owned)\n\
         }\n\
         drop Choice(&+self) { return }\n\
         func hold(value: Choice): void { return }\n",
    );
    let program = output.program();
    let choice = nominal_named(program, "Choice", &[]);
    let mut resolver = ConcreteDispatchResolver::new(program);
    let plan = resolver
        .resolve_destruction(choice, &TypeSubstitution::default())
        .unwrap()
        .unwrap();
    let ConcreteDestructionKind::Enum { drop, variants } = plan.kind() else {
        panic!("Choice must use enum destruction")
    };

    assert!(drop.is_some());
    assert_eq!(variants.len(), 1);
    assert_eq!(
        variants[0]
            .payload()
            .iter()
            .map(|payload| parameter_name(program, payload.parameter()))
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
}

#[test]
fn enum_residual_excludes_the_already_run_owner_drop_and_moved_payload() {
    let output = check(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         enum Pair { values(first: Owned, second: Owned) }\n\
         drop Pair(&+self) { return }\n\
         func consume(first: Owned, second: Owned): void {\n\
             let _ = match Pair.values(move first, move second) {\n\
                 Pair.values(item, _) { move item }\n\
             }\n\
             return\n\
         }\n",
    );
    let program = output.program();
    let (variant, payload, ty) = program
        .bodies()
        .iter()
        .flat_map(|(_, body)| {
            body.nodes()
                .iter()
                .flat_map(|(node, _)| body.cleanups().schedules(node).into_iter().flatten())
        })
        .flat_map(crate::CleanupSchedule::actions)
        .find_map(|action| match action.target() {
            CleanupTarget::EnumResidual {
                variant,
                payload,
                ty,
                ..
            } => Some((*variant, payload.clone(), *ty)),
            CleanupTarget::Path(_)
            | CleanupTarget::Place { .. }
            | CleanupTarget::Value { .. }
            | CleanupTarget::Region { .. } => None,
        })
        .expect("consuming pattern must retain its residual payload cleanup");
    let mut resolver = ConcreteDispatchResolver::new(program);
    let plan = resolver
        .resolve_enum_residual(ty, variant, &payload, &TypeSubstitution::default())
        .unwrap()
        .unwrap();
    let ConcreteDestructionKind::Enum { drop, variants } = plan.kind() else {
        panic!("residual must retain enum representation selection")
    };

    assert!(drop.is_none());
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].payload().len(), 1);
    assert_eq!(
        parameter_name(program, variants[0].payload()[0].parameter()),
        "second"
    );
}

#[test]
fn readwrite_capture_is_move_only_but_has_no_owned_drop_glue() {
    let output = check(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func hold(): void {\n\
             var value = Owned { value: 1 }\n\
             let closure = (&+value;): void { return }\n\
             return\n\
         }\n",
    );
    let program = output.program();
    let definition = program.closures().definitions().iter().next().unwrap().1;
    assert!(matches!(
        program.types().get(definition.environment()[0].ty()),
        Some(TypeKind::Borrow { .. })
    ));
    let mut resolver = ConcreteDispatchResolver::new(program);

    assert_eq!(
        resolver
            .resolve_destruction(definition.ty(), &TypeSubstitution::default())
            .unwrap(),
        None
    );
}

#[test]
fn opaque_glue_opens_the_specialized_checked_witness() {
    let output = check(
        "pub interface Show { pub method &self.show(): i32 }\n\
         struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         conform Show for Owned { method &self.show(): i32 { 1 } }\n\
         func hide<T>(value: T): some Show where T: Show { move value }\n\
         func hold(value: Owned): void {\n\
             let erased = hide(move value)\n\
             return\n\
         }\n",
    );
    let program = output.program();
    let owned = nominal_named(program, "Owned", &[]);
    let opaque = program
        .types()
        .iter()
        .find_map(|(ty, kind)| match kind {
            TypeKind::Opaque { arguments, .. } if arguments.as_ref() == [owned] => Some(ty),
            _ => None,
        })
        .expect("generic opaque result must be specialized at its call site");
    let mut resolver = ConcreteDispatchResolver::new(program);
    let plan = resolver
        .resolve_destruction(opaque, &TypeSubstitution::default())
        .unwrap()
        .unwrap();
    let ConcreteDestructionKind::Opaque { witness, plan, .. } = plan.kind() else {
        panic!("opaque destruction must retain its representation witness")
    };

    assert_eq!(*witness, owned);
    assert!(matches!(
        plan.kind(),
        ConcreteDestructionKind::Struct {
            drop: Some(_),
            fields,
        } if fields.is_empty()
    ));
}
