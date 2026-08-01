use super::{
    ABI_WORD_SIZE, AbiEnum, AbiField, AbiType, DIRECT_VALUE_MAX_SIZE, FieldLayout, LayoutError,
    StructLayout, ValueClassification, ValueLayout,
};

pub fn layout_of(ty: &AbiType) -> Result<ValueLayout, LayoutError> {
    match ty {
        AbiType::Bool | AbiType::U8 | AbiType::I8 => Ok(ValueLayout::new(1, 1)),
        AbiType::U16 | AbiType::I16 => Ok(ValueLayout::new(2, 2)),
        AbiType::U32 | AbiType::I32 => Ok(ValueLayout::new(4, 4)),
        AbiType::U64
        | AbiType::I64
        | AbiType::Usize
        | AbiType::Isize
        | AbiType::Pointer
        | AbiType::Borrow => Ok(ValueLayout::new(8, 8)),
        AbiType::StrView | AbiType::SliceView => Ok(ValueLayout::new(16, 8)),
        AbiType::Array { element, length } => layout_array(element, *length),
        AbiType::Struct(fields) => {
            let layout = layout_struct(fields)?;
            Ok(ValueLayout::new(layout.size, layout.align))
        }
        AbiType::Enum(enum_) => layout_enum(enum_),
    }
}

pub fn array_element_stride(element: &AbiType) -> Result<u64, LayoutError> {
    let layout = layout_of(element)?;
    align_to(layout.size, layout.align)
}

pub fn layout_array(element: &AbiType, length: u64) -> Result<ValueLayout, LayoutError> {
    let layout = layout_of(element)?;
    let stride = align_to(layout.size, layout.align)?;
    let size = stride
        .checked_mul(length)
        .ok_or(LayoutError::SizeOverflow)?;
    Ok(ValueLayout::new(size, layout.align))
}

pub fn layout_struct(fields: &[AbiField]) -> Result<StructLayout, LayoutError> {
    let mut offset = 0_u64;
    let mut struct_align = 1_u64;
    let mut laid_out_fields = Vec::with_capacity(fields.len());

    for field in fields {
        let layout = layout_of(&field.ty)?;
        struct_align = struct_align.max(layout.align);
        offset = align_to(offset, layout.align)?;
        laid_out_fields.push(FieldLayout {
            name: field.name.clone(),
            offset,
            layout,
        });
        offset = offset
            .checked_add(layout.size)
            .ok_or(LayoutError::SizeOverflow)?;
    }

    let size = align_to(offset, struct_align)?;
    Ok(StructLayout {
        size,
        align: struct_align,
        fields: laid_out_fields,
    })
}

pub fn layout_enum(enum_: &AbiEnum) -> Result<ValueLayout, LayoutError> {
    let align = enum_.payload_layout.align.max(1);
    let payload_end = enum_
        .payload_offset
        .checked_add(enum_.payload_layout.size)
        .ok_or(LayoutError::SizeOverflow)?;
    let size = align_to(payload_end, align)?;
    Ok(ValueLayout::new(size, align))
}

pub fn classify_value(ty: &AbiType) -> Result<ValueClassification, LayoutError> {
    let layout = layout_of(ty)?;
    if layout.size <= DIRECT_VALUE_MAX_SIZE {
        Ok(ValueClassification::Direct {
            words: layout.size.div_ceil(ABI_WORD_SIZE) as usize,
        })
    } else {
        Ok(ValueClassification::Indirect)
    }
}

pub(in crate::abi) fn align_to(value: u64, align: u64) -> Result<u64, LayoutError> {
    if align == 0 || !align.is_power_of_two() {
        return Err(LayoutError::InvalidAlignment(align));
    }

    let mask = align - 1;
    value
        .checked_add(mask)
        .map(|sum| sum & !mask)
        .ok_or(LayoutError::SizeOverflow)
}
