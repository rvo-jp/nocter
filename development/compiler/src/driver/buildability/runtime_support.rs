use super::*;

pub(super) fn slice_index_assignment_target_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    match unwrap_group_expr(expression) {
        Expr::Identifier(identifier) => {
            let symbol = resolved.local_symbol_for_identifier(identifier)?;
            match typecheck_facts.binding_scalar_view_kind(symbol.name_span)? {
                TypecheckScalarViewKind::Slice(element) => {
                    if typecheck_slice_element_kind_is_buildable(element) {
                        return Some(true);
                    }
                    let ty = typecheck_facts.binding_type_expr(symbol.name_span)?;
                    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
                    slice_index_target_type_expr_is_buildable_for_sources(
                        &ty,
                        resolved,
                        resolved_sources,
                    )
                }
                TypecheckScalarViewKind::I32
                | TypecheckScalarViewKind::U8
                | TypecheckScalarViewKind::Usize
                | TypecheckScalarViewKind::Bool
                | TypecheckScalarViewKind::Str => None,
            }
        }
        Expr::Call(call) => {
            let return_type = call_return_type_expr_with_substitutions(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )?;
            slice_index_target_type_expr_is_buildable_for_sources(
                &return_type,
                resolved,
                resolved_sources,
            )
        }
        Expr::Member(member) => match typecheck_facts.field_scalar_view_kind(member.member_span)? {
            TypecheckScalarViewKind::Slice(element) => {
                if typecheck_slice_element_kind_is_buildable(element) {
                    return Some(true);
                }
                let ty = field_type_expr_for_member(member, resolved, typecheck_facts)?;
                let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
                slice_index_target_type_expr_is_buildable_for_sources(
                    &ty,
                    resolved,
                    resolved_sources,
                )
            }
            TypecheckScalarViewKind::I32
            | TypecheckScalarViewKind::U8
            | TypecheckScalarViewKind::Usize
            | TypecheckScalarViewKind::Bool
            | TypecheckScalarViewKind::Str => None,
        },
        Expr::Propagate(propagation) => slice_index_assignment_fallible_target_is_buildable(
            &propagation.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Force(force) => slice_index_assignment_fallible_target_is_buildable(
            &force.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Catch(catch) => slice_index_assignment_fallible_target_is_buildable(
            &catch.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Group(group) => slice_index_assignment_target_is_buildable(
            &group.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => None,
    }
}

pub(super) fn slice_index_assignment_fallible_target_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    let Expr::Call(call) = unwrap_group_expr(expression) else {
        return None;
    };
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let TypeExpr::Fallible(fallible) = return_type else {
        return None;
    };
    slice_index_target_type_expr_is_buildable_for_sources(
        &fallible.success,
        resolved,
        resolved_sources,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReturnShape {
    Void,
    Never,
    DiscardableScalar,
    DiscardableView,
    DiscardableAggregate,
    FallibleDiscardable,
    Other,
}

pub(super) fn call_return_shape(
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<ReturnShape> {
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    Some(return_shape_from_type_expr(&return_type, resolved))
}

pub(super) fn call_return_shape_for_sources(
    call: &CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<ReturnShape> {
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    Some(return_shape_from_type_expr_for_sources(
        &return_type,
        resolved,
        resolved_sources,
    ))
}

pub(super) fn return_shape_from_type_expr(ty: &TypeExpr, resolved: &ResolveOutput) -> ReturnShape {
    return_shape_from_type_expr_with_resolver(ty, resolved, &|_| Some(resolved))
}

pub(super) fn return_shape_from_type_expr_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> ReturnShape {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    return_shape_from_type_expr_with_resolver(ty, fallback_resolved, &source_resolver)
}

pub(super) fn return_shape_from_type_expr_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> ReturnShape
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    return_shape_from_type_expr_inner(ty, fallback_resolved, resolver, &mut HashSet::new())
}

pub(super) fn return_shape_from_type_expr_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> ReturnShape
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Reference(reference) if reference.name == "void" => ReturnShape::Void,
        TypeExpr::Reference(reference) if reference.name == "never" => ReturnShape::Never,
        TypeExpr::Reference(reference)
            if matches!(reference.name.as_str(), "i32" | "u8" | "usize" | "bool") =>
        {
            ReturnShape::DiscardableScalar
        }
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && type_expr_resolves_to_str_with_resolver(
                    &borrow.inner,
                    fallback_resolved,
                    resolver,
                ) =>
        {
            ReturnShape::DiscardableView
        }
        TypeExpr::Borrow(borrow)
            if type_expr_resolves_to_supported_slice_view_with_resolver(
                &borrow.inner,
                fallback_resolved,
                resolver,
            )
            .unwrap_or(false) =>
        {
            ReturnShape::DiscardableView
        }
        _ if type_expr_is_supported_aggregate_return_with_resolver(
            ty,
            fallback_resolved,
            resolver,
        ) =>
        {
            ReturnShape::DiscardableAggregate
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let Some(symbol) = type_symbol_by_reference_name(resolved, &reference.name) else {
                return ReturnShape::Other;
            };
            let Some(target) = &symbol.alias_target else {
                return ReturnShape::Other;
            };
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return ReturnShape::Other;
            }
            let shape = return_shape_from_type_expr_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            shape
        }
        TypeExpr::Fallible(fallible) => {
            match return_shape_from_type_expr_inner(
                &fallible.success,
                fallback_resolved,
                resolver,
                resolving_names,
            ) {
                ReturnShape::Void
                | ReturnShape::DiscardableScalar
                | ReturnShape::DiscardableView
                | ReturnShape::DiscardableAggregate => ReturnShape::FallibleDiscardable,
                ReturnShape::Never | ReturnShape::FallibleDiscardable | ReturnShape::Other => {
                    ReturnShape::Other
                }
            }
        }
        _ => ReturnShape::Other,
    }
}

pub(super) fn slice_index_expression_is_buildable(
    expression: &crate::ast::IndexExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    slice_index_target_is_buildable(
        &expression.object,
        resolved,
        resolved_sources,
        typecheck_facts,
        generic_substitutions,
    )
}

pub(super) fn slice_index_target_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<bool> {
    match unwrap_group_expr(expression) {
        Expr::StringLiteral(_) => Some(true),
        Expr::Identifier(identifier) => {
            let symbol = resolved.local_symbol_for_identifier(identifier)?;
            match typecheck_facts.binding_scalar_view_kind(symbol.name_span)? {
                TypecheckScalarViewKind::Str => Some(true),
                TypecheckScalarViewKind::Slice(element) => {
                    if typecheck_slice_element_kind_is_buildable(element) {
                        return Some(true);
                    }
                    let ty = typecheck_facts.binding_type_expr(symbol.name_span)?;
                    let ty = substitute_type_expr_parameters(ty, generic_substitutions);
                    slice_index_target_type_expr_is_buildable_for_sources(
                        &ty,
                        resolved,
                        resolved_sources,
                    )
                }
                TypecheckScalarViewKind::I32
                | TypecheckScalarViewKind::U8
                | TypecheckScalarViewKind::Usize
                | TypecheckScalarViewKind::Bool => None,
            }
        }
        Expr::Call(call) => {
            let return_type = call_return_type_expr_with_substitutions(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )?;
            slice_index_target_type_expr_is_buildable_for_sources(
                &return_type,
                resolved,
                resolved_sources,
            )
        }
        Expr::Member(member) => match typecheck_facts.field_scalar_view_kind(member.member_span)? {
            TypecheckScalarViewKind::Str => Some(true),
            TypecheckScalarViewKind::Slice(element) => {
                if typecheck_slice_element_kind_is_buildable(element) {
                    return Some(true);
                }
                let ty = field_type_expr_for_member(member, resolved, typecheck_facts)?;
                let ty = substitute_type_expr_parameters(&ty, generic_substitutions);
                slice_index_target_type_expr_is_buildable_for_sources(
                    &ty,
                    resolved,
                    resolved_sources,
                )
            }
            TypecheckScalarViewKind::I32
            | TypecheckScalarViewKind::U8
            | TypecheckScalarViewKind::Usize
            | TypecheckScalarViewKind::Bool => None,
        },
        Expr::Group(group) => slice_index_target_is_buildable(
            &group.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => None,
    }
}

pub(super) fn slice_index_assignment_element_kind(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypecheckSliceElementKind> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    match unwrap_group_expr(expression) {
        Expr::Identifier(identifier) => {
            let symbol = resolved.local_symbol_for_identifier(identifier)?;
            match typecheck_facts.binding_scalar_view_kind(symbol.name_span)? {
                TypecheckScalarViewKind::Slice(element) => Some(element),
                TypecheckScalarViewKind::I32
                | TypecheckScalarViewKind::U8
                | TypecheckScalarViewKind::Usize
                | TypecheckScalarViewKind::Bool
                | TypecheckScalarViewKind::Str => None,
            }
        }
        Expr::Call(call) => {
            let return_type = call_return_type_expr_with_substitutions(
                call,
                resolved,
                typecheck_facts,
                generic_substitutions,
            )?;
            slice_index_target_type_expr_element_kind_with_resolver(
                &return_type,
                resolved,
                &source_resolver,
            )
        }
        Expr::Member(member) => match typecheck_facts.field_scalar_view_kind(member.member_span)? {
            TypecheckScalarViewKind::Slice(element) => Some(element),
            TypecheckScalarViewKind::I32
            | TypecheckScalarViewKind::U8
            | TypecheckScalarViewKind::Usize
            | TypecheckScalarViewKind::Bool
            | TypecheckScalarViewKind::Str => None,
        },
        Expr::Propagate(propagation) => slice_index_assignment_fallible_element_kind(
            &propagation.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Force(force) => slice_index_assignment_fallible_element_kind(
            &force.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Catch(catch) => slice_index_assignment_fallible_element_kind(
            &catch.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        Expr::Group(group) => slice_index_assignment_element_kind(
            &group.expression,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        ),
        _ => None,
    }
}

pub(super) fn slice_index_assignment_fallible_element_kind(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypecheckSliceElementKind> {
    let Expr::Call(call) = unwrap_group_expr(expression) else {
        return None;
    };
    let return_type = call_return_type_expr_with_substitutions(
        call,
        resolved,
        typecheck_facts,
        generic_substitutions,
    )?;
    let TypeExpr::Fallible(fallible) = return_type else {
        return None;
    };
    let source_resolver = |source| resolved_sources.get(&source).copied();
    slice_index_target_type_expr_element_kind_with_resolver(
        &fallible.success,
        resolved,
        &source_resolver,
    )
}

pub(super) fn call_return_type_expr_with_substitutions(
    call: &CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> Option<TypeExpr> {
    if let Expr::Member(member) = call.callee.as_ref() {
        if let Some(specialization) =
            concrete_method_call_specialization(member, typecheck_facts, generic_substitutions)
        {
            let signature =
                resolved.method_signature_by_name_span(specialization.declaration_span)?;
            let mut return_type = signature.signature.return_type.clone();
            let self_substitution =
                HashMap::from([("Self".to_string(), specialization.self_ty.clone())]);
            return_type = substitute_type_expr_parameters(&return_type, &self_substitution);
            return_type =
                substitute_type_expr_parameters(&return_type, &specialization.substitutions);
            return Some(substitute_type_expr_parameters(
                &return_type,
                generic_substitutions,
            ));
        }

        if let Some(method_name_span) = typecheck_facts.method_call_target(member.member_span) {
            if typecheck_facts
                .generic_method_call_target(member.member_span)
                .is_some()
            {
                return None;
            }
            let method = resolved.method_signature_by_name_span(method_name_span)?;
            let mut return_type = method.signature.return_type.clone();
            if let Some(self_ty) = &method.impl_target_ty {
                let self_substitution = HashMap::from([("Self".to_string(), self_ty.clone())]);
                return_type = substitute_type_expr_parameters(&return_type, &self_substitution);
            }
            return Some(substitute_type_expr_parameters(
                &return_type,
                generic_substitutions,
            ));
        }
    }

    let signature = resolved.call_signature_for_call(call)?;
    let mut return_type = signature.return_type.clone();

    if let Some(specialization) =
        concrete_function_call_specialization(call, typecheck_facts, generic_substitutions)
    {
        return_type = substitute_type_expr_parameters(&return_type, &specialization.substitutions);
    }

    Some(substitute_type_expr_parameters(
        &return_type,
        generic_substitutions,
    ))
}

pub(super) fn slice_index_target_type_expr_is_buildable_for_sources(
    ty: &TypeExpr,
    fallback_resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<bool> {
    let source_resolver = |source| resolved_sources.get(&source).copied();
    slice_index_target_type_expr_is_buildable_with_resolver(ty, fallback_resolved, &source_resolver)
}

pub(super) fn slice_index_target_type_expr_is_buildable_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<bool>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    slice_index_target_type_expr_is_buildable_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

pub(super) fn slice_index_target_type_expr_is_buildable_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<bool>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && type_expr_resolves_to_str_with_resolver(
                    &borrow.inner,
                    fallback_resolved,
                    resolver,
                ) =>
        {
            Some(true)
        }
        TypeExpr::Borrow(borrow) => type_expr_resolves_to_supported_slice_view_with_resolver(
            &borrow.inner,
            fallback_resolved,
            resolver,
        ),
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = slice_index_target_type_expr_is_buildable_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

pub(super) fn slice_index_target_type_expr_element_kind_with_resolver<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
) -> Option<TypecheckSliceElementKind>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    slice_index_target_type_expr_element_kind_inner(
        ty,
        fallback_resolved,
        resolver,
        &mut HashSet::new(),
    )
}

pub(super) fn slice_index_target_type_expr_element_kind_inner<'a, F>(
    ty: &TypeExpr,
    fallback_resolved: &'a ResolveOutput,
    resolver: &F,
    resolving_names: &mut HashSet<String>,
) -> Option<TypecheckSliceElementKind>
where
    F: Fn(SourceId) -> Option<&'a ResolveOutput>,
{
    match ty {
        TypeExpr::Borrow(borrow) => {
            if !borrow.is_readwrite
                && type_expr_resolves_to_str_with_resolver(
                    &borrow.inner,
                    fallback_resolved,
                    resolver,
                )
            {
                return Some(TypecheckSliceElementKind::Str);
            }
            type_expr_resolved_view_element_kind_with_resolver(
                &borrow.inner,
                fallback_resolved,
                resolver,
            )
        }
        TypeExpr::Reference(reference) => {
            let resolved = resolved_for_type_expr(ty, fallback_resolved, resolver);
            let symbol = type_symbol_by_reference_name(resolved, &reference.name)?;
            let target = symbol.alias_target.as_ref()?;
            if !resolving_names.insert(symbol.canonical_name.clone()) {
                return None;
            }
            let result = slice_index_target_type_expr_element_kind_inner(
                target,
                fallback_resolved,
                resolver,
                resolving_names,
            );
            resolving_names.remove(&symbol.canonical_name);
            result
        }
        _ => None,
    }
}

pub(super) fn unsupported_std_vec_element_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    nocter_home: Option<&Path>,
) -> Option<Diagnostic> {
    let element_use = std_vec_element_use(
        sources,
        call,
        typecheck_facts,
        generic_substitutions,
        nocter_home,
    )?;
    let is_supported = match element_use.operation {
        StdVecElementOperation::OwnedStorage => type_expr_is_supported_std_vec_element_storage(
            &element_use.ty,
            resolved,
            resolved_sources,
        ),
        StdVecElementOperation::CopyFromSlice => {
            type_expr_is_supported_std_vec_copy_element_storage(
                &element_use.ty,
                resolved,
                resolved_sources,
            )
        }
    };
    if is_supported {
        return None;
    }

    let (feature, help) = match element_use.operation {
        StdVecElementOperation::OwnedStorage => (
            "`Vec` element storage without runtime-supported recursive drop glue",
            "use a scalar, `&str`, fixed array, or struct element with a supported ABI layout",
        ),
        StdVecElementOperation::CopyFromSlice => (
            "`Vec.from_slice` with a non-copy element type",
            "move owned elements into a Vec with `push`; `from_slice` duplicates every source element",
        ),
    };
    Some(unsupported_v0_build_diagnostic(
        sources, call.span, feature, help,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StdVecElementOperation {
    OwnedStorage,
    CopyFromSlice,
}

struct StdVecElementUse {
    operation: StdVecElementOperation,
    ty: TypeExpr,
}

fn std_vec_element_use(
    sources: &SourceMap,
    call: &CallExpr,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    nocter_home: Option<&Path>,
) -> Option<StdVecElementUse> {
    if let Expr::Member(member) = call.callee.as_ref()
        && let Some(specialization) =
            concrete_method_call_specialization(member, typecheck_facts, generic_substitutions)
        && source_is_std_vec(sources, specialization.declaration_span.source, nocter_home)
        && matches!(
            declaration_name(sources, specialization.declaration_span),
            Some("push" | "reserve")
        )
    {
        return specialization
            .substitutions
            .get("T")
            .cloned()
            .map(|ty| StdVecElementUse {
                operation: StdVecElementOperation::OwnedStorage,
                ty,
            });
    }

    let specialization =
        concrete_function_call_specialization(call, typecheck_facts, generic_substitutions)?;
    if !source_is_std_vec(sources, specialization.declaration_span.source, nocter_home) {
        return None;
    }
    let operation = match declaration_name(sources, specialization.declaration_span)? {
        "from_slice" => StdVecElementOperation::CopyFromSlice,
        "push" | "with_capacity" | "reserve" => StdVecElementOperation::OwnedStorage,
        _ => return None,
    };
    specialization
        .substitutions
        .get("T")
        .cloned()
        .map(|ty| StdVecElementUse { operation, ty })
}

pub(super) fn unsupported_null_from_addr_call_diagnostic(
    sources: &SourceMap,
    call: &CallExpr,
    resolved: &ResolveOutput,
    nocter_home: Option<&Path>,
) -> Option<Diagnostic> {
    let symbol = resolved.symbol_for_call(call)?;
    if !matches!(symbol.kind, SymbolKind::Primitive(_)) {
        return None;
    }
    if !source_is_std_ptr(sources, symbol.declaration_span.source, nocter_home) {
        return None;
    }
    if declaration_name(sources, symbol.declaration_span)? != "from_addr" {
        return None;
    }
    let argument = call.arguments.first()?;
    if !expression_is_statically_zero_integer(argument) {
        return None;
    }

    Some(unsupported_v0_build_diagnostic(
        sources,
        argument.span(),
        "null raw pointer construction",
        "`*T` is non-null in v0; use `none` for `*T?` absence or pass a non-zero trusted address",
    ))
}

pub(super) fn expression_is_statically_zero_integer(expression: &Expr) -> bool {
    match unwrap_group_expr(expression) {
        Expr::IntegerLiteral(literal) => decode_integer_literal_value(&literal.value) == Some(0),
        Expr::TypeConversion(conversion) => {
            expression_is_statically_zero_integer(&conversion.expression)
        }
        _ => false,
    }
}

pub(super) fn declaration_name(sources: &SourceMap, span: ByteSpan) -> Option<&str> {
    sources.get(span.source)?.text().get(span.start..span.end)
}

pub(super) fn source_is_std_ptr(
    sources: &SourceMap,
    source: SourceId,
    nocter_home: Option<&Path>,
) -> bool {
    source_is_std_module(sources, source, nocter_home, Path::new("std/ptr.nct"))
}

pub(super) fn source_is_std_vec(
    sources: &SourceMap,
    source: SourceId,
    nocter_home: Option<&Path>,
) -> bool {
    source_is_std_module(sources, source, nocter_home, Path::new("std/vec.nct"))
}

pub(super) fn source_is_std_module(
    sources: &SourceMap,
    source: SourceId,
    nocter_home: Option<&Path>,
    relative_path: &Path,
) -> bool {
    let Some(nocter_home) = nocter_home else {
        return false;
    };

    sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .and_then(|path| path.strip_prefix(nocter_home).ok())
        .is_some_and(|relative| relative == relative_path)
}

pub(super) fn method_call_receiver_is_readwrite_borrow(
    member_span: ByteSpan,
    typecheck_facts: &TypecheckFacts,
) -> bool {
    matches!(
        typecheck_facts.method_call_receiver_kind(member_span),
        Some(TypecheckMethodReceiverKind::ReadwriteBorrow)
    )
}

pub(super) fn readwrite_borrow_argument_source_is_buildable(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(_) => true,
        Expr::Member(member) => aggregate_member_root_is_identifier(&member.object),
        Expr::Index(index) => slice_index_assignment_element_kind(
            &index.object,
            resolved,
            resolved_sources,
            typecheck_facts,
            generic_substitutions,
        )
        .is_some_and(typecheck_slice_element_kind_is_buildable),
        _ => false,
    }
}

pub(super) fn aggregate_member_root_is_identifier(expression: &Expr) -> bool {
    match unwrap_group_expr(expression) {
        Expr::Identifier(_) => true,
        Expr::Member(member) => aggregate_member_root_is_identifier(&member.object),
        _ => false,
    }
}

pub(super) fn call_target_for_call(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    typecheck_facts: &TypecheckFacts,
    generic_substitutions: &HashMap<String, TypeExpr>,
    root_source: SourceId,
    names: &HashMap<ByteSpan, String>,
) -> Option<CallTarget> {
    if let Some(specialization) =
        concrete_function_call_specialization(call, typecheck_facts, generic_substitutions)
    {
        return Some(call_target_for_source(
            specialization.declaration_span.source,
            root_source,
            specialization.target_name.clone(),
        ));
    }

    if let Expr::Member(member) = call.callee.as_ref() {
        if let Some(method_name_span) = typecheck_facts.method_call_target(member.member_span) {
            let target_name = if typecheck_facts
                .generic_method_call_target(member.member_span)
                .is_some()
            {
                concrete_method_call_specialization(member, typecheck_facts, generic_substitutions)?
                    .target_name
            } else {
                names.get(&method_name_span).cloned()?
            };
            return Some(call_target_for_source(
                method_name_span.source,
                root_source,
                target_name,
            ));
        }
        if let Some((_owner, function)) = resolved.associated_function_for_call(call) {
            return Some(call_target_for_source(
                function.name_span.source,
                root_source,
                function.target_name.clone(),
            ));
        }
    }

    let Expr::Identifier(_) = call.callee.as_ref() else {
        return None;
    };
    let symbol = resolved.symbol_for_call(call)?;
    match &symbol.kind {
        SymbolKind::Function(_) | SymbolKind::Primitive(_) | SymbolKind::Type(_) => {
            let target_name = if symbol.declaration_span.source != root_source {
                names
                    .get(&symbol.declaration_span)
                    .cloned()
                    .unwrap_or_else(|| symbol.name.clone())
            } else {
                symbol.name.clone()
            };
            Some(call_target_for_source(
                symbol.declaration_span.source,
                root_source,
                target_name,
            ))
        }
        SymbolKind::Imported(_) => None,
    }
}
