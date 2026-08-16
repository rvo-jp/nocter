use crate::{CallableKind, CallableOwner, DeclarationProgram, NominalShape};

use super::{DeclarationRule, DeclarationViolation};

pub(super) fn validate(program: &DeclarationProgram) -> Result<(), DeclarationViolation> {
    validate_nonempty_enums(program)?;
    validate_complete_conformances(program)?;
    validate_opaque_results(program)
}

fn validate_nonempty_enums(program: &DeclarationProgram) -> Result<(), DeclarationViolation> {
    for (_, nominal) in program.declarations().nominal_types().iter() {
        if matches!(nominal.shape(), NominalShape::Enum { variants } if variants.is_empty()) {
            return Err(DeclarationViolation::new(
                DeclarationRule::EmptyEnum,
                nominal.site(),
            ));
        }
    }
    Ok(())
}

fn validate_complete_conformances(
    program: &DeclarationProgram,
) -> Result<(), DeclarationViolation> {
    for (_, conformance) in program.declarations().conformances().iter() {
        let Some(interface) = program
            .declarations()
            .interfaces()
            .get(conformance.interface().interface())
        else {
            continue;
        };
        if conformance.associated_types().len() != interface.associated_types().len() {
            return Err(DeclarationViolation::with_related(
                DeclarationRule::IncompleteAssociatedTypes,
                conformance.site(),
                interface.site(),
            ));
        }
    }
    Ok(())
}

fn validate_opaque_results(program: &DeclarationProgram) -> Result<(), DeclarationViolation> {
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
            return Err(DeclarationViolation::new(
                DeclarationRule::InvalidOpaqueResult,
                callable.site(),
            ));
        }
    }
    Ok(())
}
