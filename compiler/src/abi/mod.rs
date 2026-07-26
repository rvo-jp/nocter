//! Nocter ABI lowering and layout rules.

use crate::ast::{TypeExpr, substitute_type_expr_parameters, type_expr_display_lossy};
use crate::resolve::{FunctionSignature, ResolveOutput, TypeSymbol, TypeSymbolKind};
use crate::source::SourceId;
use std::collections::{HashMap, HashSet};

pub const ABI_WORD_SIZE: u64 = 8;
pub const ARGUMENT_REGISTER_COUNT: usize = 8;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterPassing {
    Direct { words: usize },
    IndirectPointer,
}

impl ParameterPassing {
    pub fn abi_word_count(self) -> usize {
        match self {
            Self::Direct { words } => words,
            Self::IndirectPointer => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnPassing {
    Void,
    Never,
    Direct { words: usize },
    IndirectPointer,
}

impl ReturnPassing {
    pub fn description(self) -> &'static str {
        match self {
            Self::Void => "void",
            Self::Never => "never",
            Self::Direct { words: 1 } => "1 direct ABI word",
            Self::Direct { words: 2 } => "2 direct ABI words",
            Self::Direct { .. } => "direct ABI words",
            Self::IndirectPointer => "an indirect return pointer",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiValue {
    pub ty: AbiType,
    pub layout: ValueLayout,
    pub classification: ValueClassification,
}

impl AbiValue {
    fn from_abi_type(ty: AbiType) -> Result<Self, AbiTypeError> {
        let layout = layout_of(&ty)?;
        let classification = classify_value(&ty)?;
        Ok(Self {
            ty,
            layout,
            classification,
        })
    }

    pub fn parameter_passing(&self) -> ParameterPassing {
        match self.classification {
            ValueClassification::Direct { words } => ParameterPassing::Direct { words },
            ValueClassification::Indirect => ParameterPassing::IndirectPointer,
        }
    }

    pub fn parameter_abi_word_count(&self) -> usize {
        self.parameter_passing().abi_word_count()
    }

    pub fn is_indirect(&self) -> bool {
        self.classification == ValueClassification::Indirect
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiParameter {
    pub name: String,
    pub value: AbiValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiReturn {
    Void,
    Never,
    Value(AbiValue),
}

impl AbiReturn {
    pub fn passing(&self) -> ReturnPassing {
        match self {
            Self::Void => ReturnPassing::Void,
            Self::Never => ReturnPassing::Never,
            Self::Value(value) => match value.classification {
                ValueClassification::Direct { words } => ReturnPassing::Direct { words },
                ValueClassification::Indirect => ReturnPassing::IndirectPointer,
            },
        }
    }

    pub fn uses_indirect_pointer(&self) -> bool {
        self.passing() == ReturnPassing::IndirectPointer
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionAbi {
    pub parameters: Vec<AbiParameter>,
    pub return_value: AbiReturn,
}

impl FunctionAbi {
    pub fn parameter_abi_word_count(&self) -> usize {
        self.parameters
            .iter()
            .map(|parameter| parameter.value.parameter_abi_word_count())
            .sum()
    }

    pub fn parameters_fit_registers(&self) -> bool {
        self.parameter_abi_word_count() <= ARGUMENT_REGISTER_COUNT
    }

    pub fn uses_indirect_return_pointer(&self) -> bool {
        self.return_value.uses_indirect_pointer()
    }
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

pub fn abi_value_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Result<AbiValue, AbiTypeError> {
    abi_value_from_type_expr_with_resolver(ty, resolved, |_| Some(resolved))
}

pub fn abi_value_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<AbiValue, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_value_from_type_expr_inner(ty, fallback_resolved, &resolver)
}

pub fn function_abi_from_signature(
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> Result<FunctionAbi, AbiTypeError> {
    function_abi_from_signature_with_resolver(signature, resolved, |_| Some(resolved))
}

pub fn function_abi_from_signature_with_resolver<'a, F>(
    signature: &FunctionSignature,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<FunctionAbi, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let parameters =
        function_parameters_abi_from_signature_inner(signature, fallback_resolved, &resolver)?;
    let return_value =
        abi_return_from_type_expr(&signature.return_type, fallback_resolved, &resolver)?;

    Ok(FunctionAbi {
        parameters,
        return_value,
    })
}

pub fn function_parameters_abi_from_signature(
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> Result<Vec<AbiParameter>, AbiTypeError> {
    function_parameters_abi_from_signature_with_resolver(signature, resolved, |_| Some(resolved))
}

pub fn function_parameters_abi_from_signature_with_resolver<'a, F>(
    signature: &FunctionSignature,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<Vec<AbiParameter>, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    function_parameters_abi_from_signature_inner(signature, fallback_resolved, &resolver)
}

fn function_parameters_abi_from_signature_inner<'a, F>(
    signature: &FunctionSignature,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Result<Vec<AbiParameter>, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    signature
        .parameters
        .iter()
        .map(|parameter| {
            Ok(AbiParameter {
                name: parameter.name.clone(),
                value: abi_value_from_type_expr_inner(&parameter.ty, fallback_resolved, resolver)?,
            })
        })
        .collect()
}

pub fn function_parameter_abi_word_count_from_signature(
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> Result<usize, AbiTypeError> {
    function_parameter_abi_word_count_from_signature_with_resolver(signature, resolved, |_| {
        Some(resolved)
    })
}

pub fn function_parameter_abi_word_count_from_signature_with_resolver<'a, F>(
    signature: &FunctionSignature,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<usize, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let mut count = 0_usize;
    for parameter in &signature.parameters {
        if type_expr_resolves_to_error(
            &parameter.ty,
            fallback_resolved,
            &resolver,
            &mut HashSet::new(),
        ) {
            count = count
                .checked_add(4)
                .ok_or(AbiTypeError::Layout(LayoutError::SizeOverflow))?;
            continue;
        }

        let parameter = AbiParameter {
            name: parameter.name.clone(),
            value: abi_value_from_type_expr_inner(&parameter.ty, fallback_resolved, &resolver)?,
        };
        count = count
            .checked_add(parameter.value.parameter_abi_word_count())
            .ok_or(AbiTypeError::Layout(LayoutError::SizeOverflow))?;
    }
    Ok(count)
}

fn type_expr_resolves_to_error<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> bool
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    let TypeExpr::Reference(reference) = ty else {
        return false;
    };

    if reference.name == "error" {
        return true;
    }

    let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
    let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
        return false;
    };
    let Some(target) = &symbol.alias_target else {
        return false;
    };
    if !resolving_names.insert(symbol.canonical_name.clone()) {
        return false;
    }
    let result = type_expr_resolves_to_error(target, fallback_resolved, resolver, resolving_names);
    resolving_names.remove(&symbol.canonical_name);
    result
}

pub fn function_success_return_passing_from_signature(
    signature: &FunctionSignature,
    resolved: &ResolveOutput,
) -> Result<ReturnPassing, AbiTypeError> {
    function_success_return_passing_from_signature_with_resolver(signature, resolved, |_| {
        Some(resolved)
    })
}

pub fn function_success_return_passing_from_signature_with_resolver<'a, F>(
    signature: &FunctionSignature,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<ReturnPassing, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    top_level_success_return_passing_from_type_expr(
        &signature.return_type,
        fallback_resolved,
        &resolver,
        &mut HashSet::new(),
    )
}

fn top_level_success_return_passing_from_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Result<ReturnPassing, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return abi_return_from_type_expr(ty, fallback_resolved, resolver)
                    .map(|return_value| return_value.passing());
            };
            let Some(target) = &symbol.alias_target else {
                return abi_return_from_type_expr(ty, fallback_resolved, resolver)
                    .map(|return_value| return_value.passing());
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return Err(AbiTypeError::RecursiveType(symbol.canonical_name.clone()));
            }
            let result = top_level_success_return_passing_from_type_expr(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        TypeExpr::Fallible(fallible) => {
            abi_return_from_type_expr(&fallible.success, fallback_resolved, resolver)
                .map(|return_value| return_value.passing())
        }
        TypeExpr::Optional(optional) => {
            abi_return_from_type_expr(&optional.inner, fallback_resolved, resolver)
                .map(|return_value| return_value.passing())
        }
        _ => abi_return_from_type_expr(ty, fallback_resolved, resolver)
            .map(|return_value| return_value.passing()),
    }
}

pub fn abi_type_from_type_expr(
    ty: &TypeExpr,
    resolved: &ResolveOutput,
) -> Result<AbiType, AbiTypeError> {
    abi_type_from_type_expr_with_resolver(ty, resolved, |_| Some(resolved))
}

pub fn abi_type_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: F,
) -> Result<AbiType, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match abi_type_kind_from_type_expr(
        ty,
        fallback_resolved,
        &resolver,
        &HashMap::new(),
        &mut HashSet::new(),
    )? {
        AbiTypeKind::Value(ty) => Ok(ty),
        AbiTypeKind::UnsizedStr => Err(AbiTypeError::UnsizedValue("str".to_string())),
        AbiTypeKind::UnsizedArray => Err(AbiTypeError::UnsizedValue(type_expr_display_lossy(ty))),
    }
}

fn abi_value_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Result<AbiValue, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    AbiValue::from_abi_type(abi_type_from_type_expr_with_resolver_inner(
        ty,
        fallback_resolved,
        resolver,
    )?)
}

fn abi_type_from_type_expr_with_resolver_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Result<AbiType, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match abi_type_kind_from_type_expr(
        ty,
        fallback_resolved,
        resolver,
        &HashMap::new(),
        &mut HashSet::new(),
    )? {
        AbiTypeKind::Value(ty) => Ok(ty),
        AbiTypeKind::UnsizedStr => Err(AbiTypeError::UnsizedValue("str".to_string())),
        AbiTypeKind::UnsizedArray => Err(AbiTypeError::UnsizedValue(type_expr_display_lossy(ty))),
    }
}

fn abi_return_from_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Result<AbiReturn, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    abi_return_from_type_expr_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

fn abi_return_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Result<AbiReturn, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if reference.name == "void" => Ok(AbiReturn::Void),
        TypeExpr::Reference(reference) if reference.name == "never" => Ok(AbiReturn::Never),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = resolved.type_symbol_by_reference_name(&reference.name) else {
                return abi_value_from_type_expr_inner(ty, fallback_resolved, resolver)
                    .map(AbiReturn::Value);
            };
            let Some(target) = &symbol.alias_target else {
                return abi_value_from_type_expr_inner(ty, fallback_resolved, resolver)
                    .map(AbiReturn::Value);
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return Err(AbiTypeError::RecursiveType(symbol.canonical_name.clone()));
            }
            let result = abi_return_from_type_expr_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => abi_value_from_type_expr_inner(ty, fallback_resolved, resolver).map(AbiReturn::Value),
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

fn resolved_for_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> &'a ResolveOutput
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    resolver(ty.span().source).unwrap_or(fallback_resolved)
}

fn abi_type_kind_from_type_expr<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> Result<AbiTypeKind, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
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
            name if substitutions.contains_key(name) => {
                let Some(substitution) = substitutions.get(name) else {
                    return Err(AbiTypeError::UnresolvedType(name.to_string()));
                };
                let substitution = substitute_type_expr_parameters(substitution, substitutions);
                abi_type_kind_from_type_expr(
                    &substitution,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )
            }
            name => {
                let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
                let Some(symbol) = resolved.type_symbol_by_reference_name(name) else {
                    return Err(AbiTypeError::UnresolvedType(name.to_string()));
                };
                if symbol.generic_arity > 0 {
                    return Err(AbiTypeError::UnsupportedType(symbol.canonical_name.clone()));
                }
                abi_type_kind_from_symbol(
                    symbol,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )
            }
        },
        TypeExpr::Generic(generic) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = resolved.type_symbol_by_reference_name(&generic.name) else {
                return Err(AbiTypeError::UnresolvedType(generic.name.clone()));
            };
            if symbol.generic_arity != generic.arguments.len() {
                return Err(AbiTypeError::UnsupportedType(type_expr_display_lossy(ty)));
            }

            let mut instantiated_substitutions = substitutions.clone();
            for (parameter, argument) in symbol
                .generic_parameters
                .iter()
                .zip(generic.arguments.iter())
            {
                instantiated_substitutions.insert(
                    parameter.clone(),
                    substitute_type_expr_parameters(argument, substitutions),
                );
            }
            abi_type_kind_from_symbol(
                symbol,
                fallback_resolved,
                resolver,
                &instantiated_substitutions,
                resolving_names,
            )
        }
        TypeExpr::Pointer(_) => Ok(AbiTypeKind::Value(AbiType::Pointer)),
        TypeExpr::Borrow(borrow) => {
            match abi_type_kind_from_type_expr(
                &borrow.inner,
                fallback_resolved,
                resolver,
                substitutions,
                resolving_names,
            )? {
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

fn abi_type_kind_from_symbol<'a, F>(
    symbol: &TypeSymbol,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    substitutions: &HashMap<String, TypeExpr>,
    resolving_names: &mut HashSet<String>,
) -> Result<AbiTypeKind, AbiTypeError>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    if !resolving_names.insert(symbol.canonical_name.clone()) {
        return Err(AbiTypeError::RecursiveType(symbol.canonical_name.clone()));
    }

    let result = (|| match symbol.kind {
        TypeSymbolKind::Alias => {
            if let Some(target) = &symbol.alias_target {
                abi_type_kind_from_type_expr(
                    target,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )
            } else {
                Err(AbiTypeError::UnsupportedType(symbol.canonical_name.clone()))
            }
        }
        TypeSymbolKind::Struct => {
            let mut fields = Vec::with_capacity(symbol.fields.len());
            for field in &symbol.fields {
                let ty = match abi_type_kind_from_type_expr(
                    &field.ty,
                    fallback_resolved,
                    resolver,
                    substitutions,
                    resolving_names,
                )? {
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
        TypeSymbolKind::Enum => payloadless_enum_tag_type(symbol),
        TypeSymbolKind::Interface => {
            Err(AbiTypeError::UnsupportedType(symbol.canonical_name.clone()))
        }
    })();

    resolving_names.remove(&symbol.canonical_name);
    result
}

fn payloadless_enum_tag_type(symbol: &TypeSymbol) -> Result<AbiTypeKind, AbiTypeError> {
    if symbol
        .variants
        .iter()
        .any(|variant| !variant.payload.is_empty())
        || symbol.variants.len() > u8::MAX as usize + 1
    {
        return Err(AbiTypeError::UnsupportedType(symbol.canonical_name.clone()));
    }

    Ok(AbiTypeKind::Value(AbiType::U8))
}

#[cfg(test)]
mod tests {
    use super::{
        AbiField, AbiReturn, AbiType, ParameterPassing, ReturnPassing, ValueClassification,
        ValueLayout, abi_type_from_type_expr, abi_value_from_type_expr_with_resolver,
        classify_value, function_abi_from_signature,
        function_parameter_abi_word_count_from_signature, function_parameters_abi_from_signature,
        function_success_return_passing_from_signature, layout_of, layout_struct,
    };
    use crate::ast::{AstFile, Item, TypeExpr, substitute_type_expr_parameters};
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::resolve::{FunctionSignature, ResolveOutput, SymbolKind, resolve};
    use crate::source::SourceMap;
    use std::collections::HashMap;

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
    fn maps_concrete_generic_struct_type_expr_to_abi_struct_layout() {
        let (ast, resolved) = parse_and_resolve(
            r#"struct Box<T> {
    value: T
}

func make(): Box<i32> {
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

        assert_eq!(
            ty,
            AbiType::Struct(vec![AbiField::new("value", AbiType::I32)])
        );
        assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(4, 4));
        assert_eq!(
            classify_value(&ty).unwrap(),
            ValueClassification::Direct { words: 1 }
        );
    }

    #[test]
    fn maps_nested_concrete_generic_struct_type_expr_to_abi_struct_layout() {
        let (ast, resolved) = parse_and_resolve(
            r#"struct Pair<T, U> {
    first: T
    second: U
}

struct Box<T> {
    value: Pair<T, usize>
}

func make(): Box<i32> {
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

        assert_eq!(
            ty,
            AbiType::Struct(vec![AbiField::new(
                "value",
                AbiType::Struct(vec![
                    AbiField::new("first", AbiType::I32),
                    AbiField::new("second", AbiType::Usize),
                ])
            )])
        );
        assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(16, 8));
        assert_eq!(
            classify_value(&ty).unwrap(),
            ValueClassification::Direct { words: 2 }
        );
    }

    #[test]
    fn source_aware_abi_lays_out_generic_struct_with_foreign_type_argument() {
        let mut sources = SourceMap::new();
        let root_ast = parse_source(
            &mut sources,
            "app.nct",
            r#"struct Pair {
    left: i32
    right: usize
}

func make_pair(): Pair {
}
"#,
        );
        let library_ast = parse_source(
            &mut sources,
            "std/box.nct",
            r#"struct Box<T> {
    value: T
}

func make_box<T>(): Box<T> {
}
"#,
        );
        let root_resolved = resolve(&sources, &root_ast);
        let library_resolved = resolve(&sources, &library_ast);
        assert!(
            root_resolved.diagnostics.is_empty(),
            "{:?}",
            root_resolved.diagnostics
        );
        assert!(
            library_resolved.diagnostics.is_empty(),
            "{:?}",
            library_resolved.diagnostics
        );

        let pair_ty = function_return_type(&root_ast, "make_pair").clone();
        let box_template_ty = function_return_type(&library_ast, "make_box");
        let box_pair_ty = substitute_type_expr_parameters(
            box_template_ty,
            &HashMap::from([("T".to_string(), pair_ty)]),
        );

        let value =
            abi_value_from_type_expr_with_resolver(&box_pair_ty, &library_resolved, |source| {
                match source {
                    source if source == root_ast.span.source => Some(&root_resolved),
                    source if source == library_ast.span.source => Some(&library_resolved),
                    _ => None,
                }
            })
            .unwrap();

        assert_eq!(
            value.ty,
            AbiType::Struct(vec![AbiField::new(
                "value",
                AbiType::Struct(vec![
                    AbiField::new("left", AbiType::I32),
                    AbiField::new("right", AbiType::Usize),
                ])
            )])
        );
        assert_eq!(value.layout, ValueLayout::new(16, 8));
        assert_eq!(
            value.classification,
            ValueClassification::Direct { words: 2 }
        );
    }

    #[test]
    fn maps_payloadless_enum_type_expr_to_u8_tag_layout() {
        let (ast, resolved) = parse_and_resolve(
            r#"enum Choice {
    yes
    no
}

func choose(): Choice {
}
"#,
        );
        let return_type = ast
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == "choose" => {
                    Some(&function.return_type)
                }
                _ => None,
            })
            .expect("expected choose function");

        let ty = abi_type_from_type_expr(return_type, &resolved).unwrap();

        assert_eq!(ty, AbiType::U8);
        assert_eq!(layout_of(&ty).unwrap(), ValueLayout::new(1, 1));
        assert_eq!(
            classify_value(&ty).unwrap(),
            ValueClassification::Direct { words: 1 }
        );
    }

    #[test]
    fn rejects_payload_enum_type_expr_as_abi_value() {
        let (ast, resolved) = parse_and_resolve(
            r#"enum Status {
    missing
    found(code: i32)
}

func status(): Status {
}
"#,
        );
        let return_type = ast
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == "status" => {
                    Some(&function.return_type)
                }
                _ => None,
            })
            .expect("expected status function");

        assert!(abi_type_from_type_expr(return_type, &resolved).is_err());
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

    #[test]
    fn classifies_function_signature_values() {
        let (_ast, resolved) = parse_and_resolve(
            r#"struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

func passthrough(text: Text, view: &str, count: usize): Text {
}
"#,
        );
        let signature = resolved_function_signature(&resolved, "passthrough");

        let abi = function_abi_from_signature(signature, &resolved).unwrap();

        assert_eq!(abi.parameters.len(), 3);
        assert_eq!(abi.parameters[0].name, "text");
        assert_eq!(abi.parameters[0].value.layout, ValueLayout::new(24, 8));
        assert_eq!(
            abi.parameters[0].value.parameter_passing(),
            ParameterPassing::IndirectPointer
        );
        assert_eq!(
            abi.parameters[0].value.classification,
            ValueClassification::Indirect
        );
        assert_eq!(abi.parameters[1].name, "view");
        assert_eq!(abi.parameters[1].value.ty, AbiType::StrView);
        assert_eq!(
            abi.parameters[1].value.parameter_passing(),
            ParameterPassing::Direct { words: 2 }
        );
        assert_eq!(
            abi.parameters[1].value.classification,
            ValueClassification::Direct { words: 2 }
        );
        assert_eq!(abi.parameters[2].name, "count");
        assert_eq!(
            abi.parameters[2].value.classification,
            ValueClassification::Direct { words: 1 }
        );
        assert_eq!(abi.parameter_abi_word_count(), 4);
        assert!(abi.parameters_fit_registers());
        assert!(abi.uses_indirect_return_pointer());
        assert_eq!(abi.return_value.passing(), ReturnPassing::IndirectPointer);
        assert!(matches!(
            abi.return_value,
            AbiReturn::Value(ref value)
                if value.layout == ValueLayout::new(24, 8)
                    && value.classification == ValueClassification::Indirect
        ));
    }

    #[test]
    fn classifies_void_and_never_returns_without_value_layout() {
        let (_ast, resolved) = parse_and_resolve(
            r#"primitive stop(): never

func done(): void {
}
"#,
        );

        let stop =
            function_abi_from_signature(resolved_function_signature(&resolved, "stop"), &resolved)
                .unwrap();
        let done =
            function_abi_from_signature(resolved_function_signature(&resolved, "done"), &resolved)
                .unwrap();

        assert_eq!(stop.return_value, AbiReturn::Never);
        assert_eq!(done.return_value, AbiReturn::Void);
        assert_eq!(stop.return_value.passing(), ReturnPassing::Never);
        assert_eq!(done.return_value.passing(), ReturnPassing::Void);
        assert!(!stop.uses_indirect_return_pointer());
        assert!(!done.uses_indirect_return_pointer());
    }

    #[test]
    fn classifies_alias_void_and_never_returns_without_value_layout() {
        let (_ast, resolved) = parse_and_resolve(
            r#"type Unit = void
type Bottom = never

primitive stop(): Bottom

func done(): Unit {
}
"#,
        );

        let stop =
            function_abi_from_signature(resolved_function_signature(&resolved, "stop"), &resolved)
                .unwrap();
        let done =
            function_abi_from_signature(resolved_function_signature(&resolved, "done"), &resolved)
                .unwrap();

        assert_eq!(stop.return_value, AbiReturn::Never);
        assert_eq!(done.return_value, AbiReturn::Void);
        assert_eq!(stop.return_value.passing(), ReturnPassing::Never);
        assert_eq!(done.return_value.passing(), ReturnPassing::Void);
    }

    #[test]
    fn detects_when_parameters_exceed_register_window() {
        let (_ast, resolved) = parse_and_resolve(
            r#"func many(
    a: &str,
    b: &str,
    c: &str,
    d: &str,
    e: usize,
): void {
}
"#,
        );
        let signature = resolved_function_signature(&resolved, "many");

        let abi = function_abi_from_signature(signature, &resolved).unwrap();

        assert_eq!(abi.parameter_abi_word_count(), 9);
        assert!(!abi.parameters_fit_registers());
    }

    #[test]
    fn counts_parameters_for_fallible_return_signatures() {
        let (_ast, resolved) = parse_and_resolve(
            r#"func load(text: &str, count: usize): i32! {
}
"#,
        );
        let signature = resolved_function_signature(&resolved, "load");

        let count = function_parameter_abi_word_count_from_signature(signature, &resolved).unwrap();

        assert_eq!(count, 3);
    }

    #[test]
    fn counts_error_parameters_as_failure_payload_words() {
        let (_ast, resolved) = parse_and_resolve(
            r#"type Error = error

func relay(error: Error, tag: i32): i32! {
}
"#,
        );
        let signature = resolved_function_signature(&resolved, "relay");

        let count = function_parameter_abi_word_count_from_signature(signature, &resolved).unwrap();

        assert_eq!(count, 5);
    }

    #[test]
    fn classifies_fallible_signature_success_return_passing() {
        let (_ast, resolved) = parse_and_resolve(
            r#"struct Header {
    tag: u64
    len: u64
}

struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

func header(): Header! {
}

func text(): Text! {
}
"#,
        );

        assert_eq!(
            function_success_return_passing_from_signature(
                resolved_function_signature(&resolved, "header"),
                &resolved,
            )
            .unwrap(),
            ReturnPassing::Direct { words: 2 }
        );
        assert_eq!(
            function_success_return_passing_from_signature(
                resolved_function_signature(&resolved, "text"),
                &resolved,
            )
            .unwrap(),
            ReturnPassing::IndirectPointer
        );
    }

    #[test]
    fn classifies_optional_signature_success_return_passing() {
        let (_ast, resolved) = parse_and_resolve(
            r#"type MaybeHeader = Header?

struct Header {
    tag: u64
    len: u64
}

struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

func header(): Header? {
}

func aliased_header(): MaybeHeader {
}

func text(): Text? {
}
"#,
        );

        assert_eq!(
            function_success_return_passing_from_signature(
                resolved_function_signature(&resolved, "header"),
                &resolved,
            )
            .unwrap(),
            ReturnPassing::Direct { words: 2 }
        );
        assert_eq!(
            function_success_return_passing_from_signature(
                resolved_function_signature(&resolved, "aliased_header"),
                &resolved,
            )
            .unwrap(),
            ReturnPassing::Direct { words: 2 }
        );
        assert_eq!(
            function_success_return_passing_from_signature(
                resolved_function_signature(&resolved, "text"),
                &resolved,
            )
            .unwrap(),
            ReturnPassing::IndirectPointer
        );
    }

    #[test]
    fn classifies_parameters_without_return_layout() {
        let (_ast, resolved) = parse_and_resolve(
            r#"struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

func load(text: Text, view: &str): i32! {
}
"#,
        );
        let signature = resolved_function_signature(&resolved, "load");

        let parameters = function_parameters_abi_from_signature(signature, &resolved).unwrap();

        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].name, "text");
        assert_eq!(parameters[0].value.layout, ValueLayout::new(24, 8));
        assert_eq!(
            parameters[0].value.classification,
            ValueClassification::Indirect
        );
        assert_eq!(parameters[1].name, "view");
        assert_eq!(parameters[1].value.ty, AbiType::StrView);
        assert_eq!(
            parameters[1].value.classification,
            ValueClassification::Direct { words: 2 }
        );
    }

    fn parse_and_resolve(text: &str) -> (crate::ast::AstFile, crate::resolve::ResolveOutput) {
        let mut sources = SourceMap::new();
        let ast = parse_source(&mut sources, "app.nct", text);
        let resolved = resolve(&sources, &ast);
        assert!(
            resolved.diagnostics.is_empty(),
            "{:?}",
            resolved.diagnostics
        );
        (ast, resolved)
    }

    fn parse_source(sources: &mut SourceMap, display_path: &str, text: &str) -> AstFile {
        let source = sources.add_source(display_path, None, text);
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let parsed = parse(&sources, source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        parsed.ast.unwrap()
    }

    fn function_return_type<'a>(ast: &'a AstFile, name: &str) -> &'a TypeExpr {
        ast.items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name == name => Some(&function.return_type),
                _ => None,
            })
            .unwrap_or_else(|| panic!("expected function `{name}`"))
    }

    fn resolved_function_signature<'a>(
        resolved: &'a ResolveOutput,
        name: &str,
    ) -> &'a FunctionSignature {
        let symbol = resolved
            .symbols
            .symbol_by_name(name)
            .unwrap_or_else(|| panic!("expected symbol `{name}`"));
        match &symbol.kind {
            SymbolKind::Function(signature) | SymbolKind::Primitive(signature) => signature,
            SymbolKind::Type(_) | SymbolKind::Imported(_) => {
                panic!("expected function or primitive symbol `{name}`")
            }
        }
    }
}
