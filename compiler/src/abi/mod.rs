//! Nocter ABI lowering and layout rules.

pub const ABI_WORD_SIZE: u64 = 8;
pub const DIRECT_VALUE_MAX_SIZE: u64 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiType {
    Bool,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    Usize,
    Isize,
    Pointer,
    Borrow,
    StrView,
    SliceView,
    Struct(Vec<AbiField>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiField {
    pub name: String,
    pub ty: AbiType,
}

impl AbiField {
    pub fn new(name: impl Into<String>, ty: AbiType) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueLayout {
    pub size: u64,
    pub align: u64,
}

impl ValueLayout {
    pub fn new(size: u64, align: u64) -> Self {
        Self { size, align }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLayout {
    pub size: u64,
    pub align: u64,
    pub fields: Vec<FieldLayout>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayout {
    pub name: String,
    pub offset: u64,
    pub layout: ValueLayout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueClassification {
    Direct { words: usize },
    Indirect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    SizeOverflow,
    InvalidAlignment(u64),
}

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
        AbiType::Struct(fields) => {
            let layout = layout_struct(fields)?;
            Ok(ValueLayout::new(layout.size, layout.align))
        }
    }
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

fn align_to(value: u64, align: u64) -> Result<u64, LayoutError> {
    if align == 0 || !align.is_power_of_two() {
        return Err(LayoutError::InvalidAlignment(align));
    }

    let mask = align - 1;
    value
        .checked_add(mask)
        .map(|sum| sum & !mask)
        .ok_or(LayoutError::SizeOverflow)
}

#[cfg(test)]
mod tests {
    use super::{
        AbiField, AbiType, ValueClassification, ValueLayout, classify_value, layout_of,
        layout_struct,
    };

    #[test]
    fn lays_out_scalar_and_view_values() {
        assert_eq!(layout_of(&AbiType::Bool).unwrap(), ValueLayout::new(1, 1));
        assert_eq!(layout_of(&AbiType::U8).unwrap(), ValueLayout::new(1, 1));
        assert_eq!(layout_of(&AbiType::I32).unwrap(), ValueLayout::new(4, 4));
        assert_eq!(layout_of(&AbiType::Usize).unwrap(), ValueLayout::new(8, 8));
        assert_eq!(
            layout_of(&AbiType::Pointer).unwrap(),
            ValueLayout::new(8, 8)
        );
        assert_eq!(layout_of(&AbiType::Borrow).unwrap(), ValueLayout::new(8, 8));
        assert_eq!(
            layout_of(&AbiType::StrView).unwrap(),
            ValueLayout::new(16, 8)
        );
        assert_eq!(
            layout_of(&AbiType::SliceView).unwrap(),
            ValueLayout::new(16, 8)
        );
    }

    #[test]
    fn lays_out_struct_fields_in_declaration_order_with_padding() {
        let layout = layout_struct(&[
            AbiField::new("tag", AbiType::U8),
            AbiField::new("count", AbiType::I32),
            AbiField::new("ptr", AbiType::Pointer),
        ])
        .unwrap();

        assert_eq!(layout.size, 16);
        assert_eq!(layout.align, 8);
        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(layout.fields[1].offset, 4);
        assert_eq!(layout.fields[2].offset, 8);
    }

    #[test]
    fn classifies_values_by_direct_size_limit() {
        assert_eq!(
            classify_value(&AbiType::Usize).unwrap(),
            ValueClassification::Direct { words: 1 }
        );
        assert_eq!(
            classify_value(&AbiType::StrView).unwrap(),
            ValueClassification::Direct { words: 2 }
        );

        let string_like = AbiType::Struct(vec![
            AbiField::new("ptr", AbiType::Pointer),
            AbiField::new("len", AbiType::Usize),
            AbiField::new("capacity", AbiType::Usize),
        ]);
        assert_eq!(layout_of(&string_like).unwrap(), ValueLayout::new(24, 8));
        assert_eq!(
            classify_value(&string_like).unwrap(),
            ValueClassification::Indirect
        );
    }
}
