//! Nocter ABI lowering and layout rules.

use crate::ast::TypeExpr;
use crate::resolve::{ResolveOutput, TypeSymbol, TypeSymbolKind};
use std::collections::HashSet;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiTypeError {
    Layout(LayoutError),
    RecursiveType(String),
    UnsupportedType(String),
    UnresolvedType(String),
    UnsizedValue(String),
}

impl From<LayoutError> for AbiTypeError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
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

pub fn abi_type_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Result<AbiType, AbiTypeError> {
    match abi_type_kind_from_type_expr(ty, resolved, &mut HashSet::new())? {
        AbiTypeKind::Value(ty) => Ok(ty),
        AbiTypeKind::UnsizedStr => Err(AbiTypeError::UnsizedValue("str".to_string())),
        AbiTypeKind::UnsizedArray => Err(AbiTypeError::UnsizedValue(type_expr_display_lossy(ty))),
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

enum AbiTypeKind {
    Value(AbiType),
    UnsizedStr,
    UnsizedArray,
}

fn abi_type_kind_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Result<AbiTypeKind, AbiTypeError> {
    match ty {
        TypeExpr::Reference(reference) => match reference.name.as_str() {
            "bool" => Ok(AbiTypeKind::Value(AbiType::Bool)),
            "u8" => Ok(AbiTypeKind::Value(AbiType::U8)),
            "i8" => Ok(AbiTypeKind::Value(AbiType::I8)),
            "u16" => Ok(AbiTypeKind::Value(AbiType::U16)),
            "i16" => Ok(AbiTypeKind::Value(AbiType::I16)),
            "u32" => Ok(AbiTypeKind::Value(AbiType::U32)),
            "i32" => Ok(AbiTypeKind::Value(AbiType::I32)),
            "u64" => Ok(AbiTypeKind::Value(AbiType::U64)),
            "i64" => Ok(AbiTypeKind::Value(AbiType::I64)),
            "usize" => Ok(AbiTypeKind::Value(AbiType::Usize)),
            "isize" => Ok(AbiTypeKind::Value(AbiType::Isize)),
            "str" => Ok(AbiTypeKind::UnsizedStr),
            "void" | "never" | "error" => {
                Err(AbiTypeError::UnsupportedType(reference.name.clone()))
            }
            name => {
                let Some(symbol) = resolved.type_symbol_by_name(name) else {
                    return Err(AbiTypeError::UnresolvedType(name.to_string()));
                };
                abi_type_kind_from_symbol(symbol, resolved, resolving_names)
            }
        },
        TypeExpr::Generic(generic) => Err(AbiTypeError::UnsupportedType(format!(
            "{}<...>",
            generic.name
        ))),
        TypeExpr::Pointer(_) => Ok(AbiTypeKind::Value(AbiType::Pointer)),
        TypeExpr::Borrow(borrow) => {
            match abi_type_kind_from_type_expr(&borrow.inner, resolved, resolving_names)? {
                AbiTypeKind::UnsizedStr => Ok(AbiTypeKind::Value(AbiType::StrView)),
                AbiTypeKind::UnsizedArray => Ok(AbiTypeKind::Value(AbiType::SliceView)),
                AbiTypeKind::Value(_) => Ok(AbiTypeKind::Value(AbiType::Borrow)),
            }
        }
        TypeExpr::View(_) => Ok(AbiTypeKind::UnsizedArray),
        TypeExpr::Array(array) => Err(AbiTypeError::UnsupportedType(format!(
            "[{}; {}]",
            type_expr_display_lossy(&array.element),
            array.length.value
        ))),
        TypeExpr::Optional(optional) => Err(AbiTypeError::UnsupportedType(format!(
            "{}?",
            type_expr_display_lossy(&optional.inner)
        ))),
        TypeExpr::Fallible(fallible) => Err(AbiTypeError::UnsupportedType(format!(
            "{}!",
            type_expr_display_lossy(&fallible.success)
        ))),
    }
}

fn abi_type_kind_from_symbol(
    symbol: &TypeSymbol,
    resolved: &ResolveOutput,
    resolving_names: &mut HashSet<String>,
) -> Result<AbiTypeKind, AbiTypeError> {
    if !resolving_names.insert(symbol.canonical_name.clone()) {
        return Err(AbiTypeError::RecursiveType(symbol.canonical_name.clone()));
    }

    let result = (|| match symbol.kind {
        TypeSymbolKind::Alias => {
            if let Some(target) = &symbol.alias_target {
                abi_type_kind_from_type_expr(target, resolved, resolving_names)
            } else {
                Err(AbiTypeError::UnsupportedType(symbol.canonical_name.clone()))
            }
        }
        TypeSymbolKind::Struct => {
            let mut fields = Vec::with_capacity(symbol.fields.len());
            for field in &symbol.fields {
                let ty = match abi_type_kind_from_type_expr(&field.ty, resolved, resolving_names)? {
                    AbiTypeKind::Value(ty) => ty,
                    AbiTypeKind::UnsizedStr => {
                        return Err(AbiTypeError::UnsizedValue("str".to_string()));
                    }
                    AbiTypeKind::UnsizedArray => {
                        return Err(AbiTypeError::UnsizedValue(type_expr_display_lossy(
                            &field.ty,
                        )));
                    }
                };
                fields.push(AbiField::new(field.name.clone(), ty));
            }
            Ok(AbiTypeKind::Value(AbiType::Struct(fields)))
        }
        TypeSymbolKind::Enum | TypeSymbolKind::Trait => {
            Err(AbiTypeError::UnsupportedType(symbol.canonical_name.clone()))
        }
    })();

    resolving_names.remove(&symbol.canonical_name);
    result
}

fn type_expr_display_lossy(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Reference(reference) => reference.name.clone(),
        TypeExpr::Generic(generic) => {
            let arguments = generic
                .arguments
                .iter()
                .map(type_expr_display_lossy)
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{arguments}>", generic.name)
        }
        TypeExpr::Pointer(pointer) => format!("*{}", type_expr_display_lossy(&pointer.inner)),
        TypeExpr::Borrow(borrow) if borrow.is_readwrite => {
            format!("&+{}", type_expr_display_lossy(&borrow.inner))
        }
        TypeExpr::Borrow(borrow) => format!("&{}", type_expr_display_lossy(&borrow.inner)),
        TypeExpr::View(view) => format!("[{}]", type_expr_display_lossy(&view.element)),
        TypeExpr::Array(array) => {
            format!(
                "[{}; {}]",
                type_expr_display_lossy(&array.element),
                array.length.value
            )
        }
        TypeExpr::Optional(optional) => format!("{}?", type_expr_display_lossy(&optional.inner)),
        TypeExpr::Fallible(fallible) => format!("{}!", type_expr_display_lossy(&fallible.success)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AbiField, AbiType, ValueClassification, ValueLayout, abi_type_from_type_expr,
        classify_value, layout_of, layout_struct,
    };
    use crate::ast::Item;
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::resolve::resolve;
    use crate::source::SourceMap;

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

    #[test]
    fn maps_resolved_struct_type_expr_to_abi_struct_layout() {
        let (ast, resolved) = parse_and_resolve(
            r#"struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

func make(): Text {
}
"#,
        );
        let return_type = ast
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == "make" => Some(&function.return_type),
                _ => None,
            })
            .expect("expected make function");

        let ty = abi_type_from_type_expr(return_type, &resolved).unwrap();

        assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(24, 8));
        assert_eq!(classify_value(&ty).unwrap(), ValueClassification::Indirect);
    }

    #[test]
    fn maps_borrow_of_str_alias_to_str_view_layout() {
        let (ast, resolved) = parse_and_resolve(
            r#"type Text = str

func view(text: &Text): &Text {
}
"#,
        );
        let return_type = ast
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == "view" => Some(&function.return_type),
                _ => None,
            })
            .expect("expected view function");

        let ty = abi_type_from_type_expr(return_type, &resolved).unwrap();

        assert_eq!(ty, AbiType::StrView);
        assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(16, 8));
    }

    fn parse_and_resolve(text: &str) -> (crate::ast::AstFile, crate::resolve::ResolveOutput) {
        let mut sources = SourceMap::new();
        let source = sources.add_source("app.nct", None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let parsed = parse(&sources, source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let ast = parsed.ast.unwrap();
        let resolved = resolve(&sources, &ast);
        assert!(
            resolved.diagnostics.is_empty(),
            "{:?}",
            resolved.diagnostics
        );
        (ast, resolved)
    }
}
