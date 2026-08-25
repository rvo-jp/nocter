use crate::{CallableKind, CallableOwner, DeclarationProgram, NominalShape};

use super::DeclarationRule;
use super::outcome::{ValidationCollector, related_violation, violation};

pub(super) fn validate(program: &DeclarationProgram, collector: &mut ValidationCollector) {
    validate_nonempty_enums(program, collector);
    validate_complete_conformances(program, collector);
    validate_opaque_results(program, collector);
}

fn validate_nonempty_enums(program: &DeclarationProgram, collector: &mut ValidationCollector) {
    for (_, nominal) in program.declarations().nominal_types().iter() {
        if matches!(nominal.shape(), NominalShape::Enum { variants } if variants.is_empty()) {
            collector.reject_program_fact(violation(DeclarationRule::EmptyEnum, nominal.site()));
        }
    }
}

fn validate_complete_conformances(
    program: &DeclarationProgram,
    collector: &mut ValidationCollector,
) {
    for (id, conformance) in program.declarations().conformances().iter() {
        let Some(interface) = program
            .declarations()
            .interfaces()
            .get(conformance.interface().interface())
        else {
            continue;
        };
        if conformance.associated_types().len() != interface.associated_types().len() {
            collector.reject_conformance(
                id,
                related_violation(
                    DeclarationRule::IncompleteAssociatedTypes,
                    conformance.site(),
                    interface.site(),
                ),
            );
        }
    }
}

fn validate_opaque_results(program: &DeclarationProgram, collector: &mut ValidationCollector) {
    for (_, opaque) in program.declarations().opaque_types().iter() {
        let Some(callable) = program.declarations().callables().get(opaque.owner()) else {
            continue;
        };
        let valid_owner = callable.body().is_some()
            && matches!(
                (callable.kind(), callable.owner()),
                (CallableKind::Function, CallableOwner::Module(_))
                    | (
                        CallableKind::ConstructionFunction,
                        CallableOwner::Construction(_)
                    )
                    | (
                        CallableKind::Method,
                        CallableOwner::Instance(_) | CallableOwner::Interface(_)
                    )
            );
        if !valid_owner {
            collector.reject_program_fact(violation(
                DeclarationRule::InvalidOpaqueResult,
                callable.site(),
            ));
        }
    }
}
