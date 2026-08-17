use nocter_declarations::NominalShape;
use nocter_model::{MirBlockId, TypeKind};

use crate::validation_types::is_integer;
use crate::{
    MirBody, MirSwitchCase, MirSwitchSubject, MirSwitchValue, MirValidationEnvironment,
    MirValidationError,
};

pub(crate) fn validate_switch_subject(
    environment: &(impl MirValidationEnvironment + ?Sized),
    function: &MirBody,
    block: MirBlockId,
    subject: MirSwitchSubject,
    cases: &[MirSwitchCase],
) -> Result<(), MirValidationError> {
    let types = environment.types();
    let valid = match subject {
        MirSwitchSubject::Value(value) => {
            let ty = function
                .values()
                .get(value)
                .ok_or(MirValidationError::UnknownValue(value))?
                .ty();
            is_integer(types, ty)
                && cases
                    .iter()
                    .all(|case| matches!(case.value(), MirSwitchValue::Integer(_)))
        }
        MirSwitchSubject::Place(place) => {
            let ty = function
                .places()
                .get(place)
                .ok_or(MirValidationError::UnknownPlace(place))?
                .ty();
            match types.get(ty) {
                Some(TypeKind::Nominal { definition, .. }) => {
                    let Some(nominal) = environment.nominal_type(*definition) else {
                        return Err(MirValidationError::InvalidSwitchSubject(block));
                    };
                    matches!(nominal.shape(), NominalShape::Enum { .. })
                        && cases.iter().all(|case| {
                            let MirSwitchValue::Variant(variant) = case.value() else {
                                return false;
                            };
                            environment
                                .variant(variant)
                                .is_some_and(|variant| variant.owner() == *definition)
                        })
                }
                Some(TypeKind::Optional(_)) => cases.iter().all(|case| {
                    matches!(
                        case.value(),
                        MirSwitchValue::OptionalPresent | MirSwitchValue::OptionalAbsent
                    )
                }),
                Some(TypeKind::Fallible(_)) => cases.iter().all(|case| {
                    matches!(
                        case.value(),
                        MirSwitchValue::FallibleSuccess | MirSwitchValue::FallibleFailure
                    )
                }),
                _ => false,
            }
        }
    };
    if !valid {
        return Err(MirValidationError::InvalidSwitchSubject(block));
    }
    Ok(())
}
