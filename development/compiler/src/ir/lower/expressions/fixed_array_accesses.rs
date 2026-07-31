use super::*;

pub(in crate::ir::lower) struct FixedArrayElementAccess {
    pub(in crate::ir::lower) instructions: Vec<Instruction>,
    pub(in crate::ir::lower) source: AggregateLocation,
    pub(in crate::ir::lower) offset: u32,
    pub(in crate::ir::lower) element: AbiType,
    pub(in crate::ir::lower) out_of_bounds: bool,
    pub(in crate::ir::lower) is_readwrite: bool,
}

pub(in crate::ir::lower) struct FixedArrayElementIndexedAccess {
    pub(in crate::ir::lower) source: AggregateLocation,
    pub(in crate::ir::lower) base_offset: u32,
    pub(in crate::ir::lower) index: UsizeValue,
    pub(in crate::ir::lower) index_instructions: Vec<Instruction>,
    pub(in crate::ir::lower) length: u64,
    pub(in crate::ir::lower) stride: u32,
    pub(in crate::ir::lower) element: AbiType,
    pub(in crate::ir::lower) is_readwrite: bool,
}

pub(in crate::ir::lower) fn fixed_array_element_access(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic> + Copy,
) -> Result<Option<FixedArrayElementAccess>, Vec<Diagnostic>> {
    let Some(metadata) =
        fixed_array_access_metadata(expression, context, temporaries, unsupported_diagnostic)?
    else {
        return Ok(None);
    };
    let Some(index) = fixed_array_constant_index_value(&expression.index) else {
        return Ok(None);
    };
    if index >= u128::from(metadata.length) {
        return Ok(Some(FixedArrayElementAccess {
            instructions: metadata.instructions,
            source: metadata.source,
            offset: 0,
            element: metadata.element,
            out_of_bounds: true,
            is_readwrite: metadata.is_readwrite,
        }));
    }

    let element_offset = u64::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(u64::from(metadata.stride)))
        .ok_or_else(unsupported_diagnostic)?;
    let offset = u64::from(metadata.base_offset)
        .checked_add(element_offset)
        .and_then(|offset| u32::try_from(offset).ok())
        .ok_or_else(unsupported_diagnostic)?;
    Ok(Some(FixedArrayElementAccess {
        instructions: metadata.instructions,
        source: metadata.source,
        offset,
        element: metadata.element,
        out_of_bounds: false,
        is_readwrite: metadata.is_readwrite,
    }))
}

pub(in crate::ir::lower) fn fixed_array_element_indexed_access(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic> + Copy,
) -> Result<Option<FixedArrayElementIndexedAccess>, Vec<Diagnostic>> {
    if fixed_array_constant_index_value(&expression.index).is_some() {
        return Ok(None);
    }
    let Some(metadata) =
        fixed_array_access_metadata(expression, context, temporaries, unsupported_diagnostic)?
    else {
        return Ok(None);
    };
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut index_instructions = metadata.instructions;
    index_instructions.extend(index.instructions);
    Ok(Some(FixedArrayElementIndexedAccess {
        source: metadata.source,
        base_offset: metadata.base_offset,
        index: index.value,
        index_instructions,
        length: metadata.length,
        stride: metadata.stride,
        element: metadata.element,
        is_readwrite: metadata.is_readwrite,
    }))
}
