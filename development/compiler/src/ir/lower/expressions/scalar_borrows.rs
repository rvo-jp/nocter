//! Loads scalar values through ordinary borrow bindings.

use super::*;

pub(super) fn lower_i32_borrow_binding_to_location(
    identifier: &IdentifierExpr,
    destination: I32Location,
    context: &LoweringContext,
) -> Option<Vec<Instruction>> {
    if let Some(parameter) = context.borrow_parameter(&identifier.name)
        && parameter.inner == Type::I32
    {
        return Some(vec![Instruction::LoadI32FromPointer {
            destination,
            pointer: UsizeValue::Location(UsizeLocation::Parameter(parameter.parameter_index)),
            offset: UsizeValue::Const(0),
        }]);
    }
    if let Some((pointer, _is_readwrite, inner)) = context.borrow_local(&identifier.name)
        && inner == &Type::I32
    {
        return Some(vec![Instruction::LoadI32FromPointer {
            destination,
            pointer: UsizeValue::Location(pointer),
            offset: UsizeValue::Const(0),
        }]);
    }
    None
}

pub(super) fn lower_u8_borrow_binding_to_location(
    identifier: &IdentifierExpr,
    destination: U8Location,
    context: &LoweringContext,
) -> Option<Vec<Instruction>> {
    if let Some(parameter) = context.borrow_parameter(&identifier.name)
        && parameter.inner == Type::U8
    {
        return Some(vec![Instruction::LoadU8FromPointer {
            destination,
            pointer: UsizeValue::Location(UsizeLocation::Parameter(parameter.parameter_index)),
            offset: UsizeValue::Const(0),
        }]);
    }
    if let Some((pointer, _is_readwrite, inner)) = context.borrow_local(&identifier.name)
        && inner == &Type::U8
    {
        return Some(vec![Instruction::LoadU8FromPointer {
            destination,
            pointer: UsizeValue::Location(pointer),
            offset: UsizeValue::Const(0),
        }]);
    }
    None
}

pub(super) fn lower_usize_borrow_binding_to_location(
    identifier: &IdentifierExpr,
    destination: UsizeLocation,
    context: &LoweringContext,
) -> Option<Vec<Instruction>> {
    if let Some(parameter) = context.borrow_parameter(&identifier.name)
        && parameter.inner == Type::Usize
    {
        return Some(vec![Instruction::LoadUsizeFromPointer {
            destination,
            pointer: UsizeValue::Location(UsizeLocation::Parameter(parameter.parameter_index)),
            offset: UsizeValue::Const(0),
        }]);
    }
    if let Some((pointer, _is_readwrite, inner)) = context.borrow_local(&identifier.name)
        && inner == &Type::Usize
    {
        return Some(vec![Instruction::LoadUsizeFromPointer {
            destination,
            pointer: UsizeValue::Location(pointer),
            offset: UsizeValue::Const(0),
        }]);
    }
    None
}

pub(super) fn lower_bool_borrow_binding_to_location(
    identifier: &IdentifierExpr,
    destination: BoolLocation,
    context: &LoweringContext,
) -> Option<Vec<Instruction>> {
    if let Some(parameter) = context.borrow_parameter(&identifier.name)
        && parameter.inner == Type::Bool
    {
        return Some(vec![Instruction::LoadBoolFromPointer {
            destination,
            pointer: UsizeValue::Location(UsizeLocation::Parameter(parameter.parameter_index)),
            offset: UsizeValue::Const(0),
        }]);
    }
    if let Some((pointer, _is_readwrite, inner)) = context.borrow_local(&identifier.name)
        && inner == &Type::Bool
    {
        return Some(vec![Instruction::LoadBoolFromPointer {
            destination,
            pointer: UsizeValue::Location(pointer),
            offset: UsizeValue::Const(0),
        }]);
    }
    None
}
