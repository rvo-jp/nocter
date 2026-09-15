use nocter_model::{BorrowCapability, BuiltinType, ConstantValue, FrozenValue, TypeId, TypeKind};

use super::{DeclarationDomain, ProgramIntegrityError};
use crate::{DeclarationProgram, DeclarationValueTable};

/// Validates the complete value authority against independently frozen declaration metadata.
///
/// Keeping this boundary separate from graph integrity lets semantic construction finish types
/// before it evaluates ordinary initializer values. Only the accepted aggregate may expose both.
pub(super) fn validate(
    program: &DeclarationProgram,
    values: &DeclarationValueTable,
) -> Result<(), ProgramIntegrityError> {
    validate_constants(program, values)?;
    validate_statics(program, values)
}

fn validate_constants(
    program: &DeclarationProgram,
    values: &DeclarationValueTable,
) -> Result<(), ProgramIntegrityError> {
    for (id, constant) in program.declarations().constants().iter() {
        let value =
            values
                .constants()
                .get(id)
                .ok_or(ProgramIntegrityError::InvalidDeclarationShape(
                    DeclarationDomain::Constant,
                ))?;
        if !constant_value_matches(program, constant.ty(), value) {
            return Err(ProgramIntegrityError::InvalidDeclarationShape(
                DeclarationDomain::Constant,
            ));
        }
    }
    Ok(())
}

fn validate_statics(
    program: &DeclarationProgram,
    values: &DeclarationValueTable,
) -> Result<(), ProgramIntegrityError> {
    for (id, static_value) in program.declarations().statics().iter() {
        let value =
            values
                .statics()
                .get(id)
                .ok_or(ProgramIntegrityError::InvalidDeclarationShape(
                    DeclarationDomain::Static,
                ))?;
        if !frozen_value_matches(program, static_value.ty(), value) {
            return Err(ProgramIntegrityError::InvalidDeclarationShape(
                DeclarationDomain::Static,
            ));
        }
    }
    Ok(())
}

fn frozen_value_matches(program: &DeclarationProgram, ty: TypeId, value: &FrozenValue) -> bool {
    match (program.types().get(ty), value) {
        (_, FrozenValue::Scalar(value)) => constant_value_matches(program, ty, value),
        (Some(TypeKind::Tuple(elements)), FrozenValue::Tuple(values)) => {
            elements.iter().len() == values.len()
                && elements
                    .iter()
                    .zip(values)
                    .all(|(element, value)| frozen_value_matches(program, element, value))
        }
        (Some(TypeKind::FixedArray { element, length }), FrozenValue::FixedArray(values)) => {
            usize::try_from(*length) == Ok(values.len())
                && values
                    .iter()
                    .all(|value| frozen_value_matches(program, *element, value))
        }
        _ => false,
    }
}

fn constant_value_matches(program: &DeclarationProgram, ty: TypeId, value: &ConstantValue) -> bool {
    match value {
        ConstantValue::Bool(_) => ty == program.types().builtin(BuiltinType::Bool),
        ConstantValue::Character(value) => {
            ty == program.types().builtin(BuiltinType::Char) && char::from_u32(*value).is_some()
        }
        ConstantValue::Float32(_) => ty == program.types().builtin(BuiltinType::F32),
        ConstantValue::Float64(_) => ty == program.types().builtin(BuiltinType::F64),
        ConstantValue::Integer(value) => program
            .types()
            .get(ty)
            .and_then(|ty| match ty {
                TypeKind::Builtin(builtin) => Some(*builtin),
                _ => None,
            })
            .is_some_and(|builtin| constant_integer_fits(*value, builtin)),
        ConstantValue::Text(_) => matches!(
            program.types().get(ty),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            }) if *referent == program.types().builtin(BuiltinType::Str)
        ),
    }
}

fn constant_integer_fits(value: i128, builtin: BuiltinType) -> bool {
    match builtin {
        BuiltinType::I8 => i8::try_from(value).is_ok(),
        BuiltinType::I16 => i16::try_from(value).is_ok(),
        BuiltinType::I32 => i32::try_from(value).is_ok(),
        BuiltinType::I64 | BuiltinType::Isize => i64::try_from(value).is_ok(),
        BuiltinType::U8 => u8::try_from(value).is_ok(),
        BuiltinType::U16 => u16::try_from(value).is_ok(),
        BuiltinType::U32 => u32::try_from(value).is_ok(),
        BuiltinType::U64 | BuiltinType::Usize => u64::try_from(value).is_ok(),
        _ => false,
    }
}
