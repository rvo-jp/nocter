use super::*;

pub(in crate::typecheck) fn type_to_type_expr_allowing_parameters(
    ty: &Type,
    span: ByteSpan,
    free_type_parameters: &mut HashSet<String>,
) -> Option<TypeExpr> {
    type_to_type_expr_inner(ty, span, Some(free_type_parameters))
}

pub(super) fn type_to_type_expr_inner(
    ty: &Type,
    span: ByteSpan,
    mut free_type_parameters: Option<&mut HashSet<String>>,
) -> Option<TypeExpr> {
    match ty {
        Type::Callable(callable) => {
            let parameters = callable
                .parameters
                .iter()
                .map(|parameter| {
                    Some(crate::ast::CallableTypeParameter {
                        span,
                        name: parameter.name.clone(),
                        name_span: parameter.name_span,
                        ty: type_to_type_expr_inner(
                            &parameter.ty,
                            span,
                            free_type_parameters.as_deref_mut(),
                        )?,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            let return_type = type_to_type_expr_inner(
                &callable.return_type,
                span,
                free_type_parameters.as_deref_mut(),
            )?;
            Some(TypeExpr::Callable(crate::ast::CallableTypeExpr {
                span: callable.span,
                func_span: callable.span,
                result_allocation: callable.result_may_allocate.then_some(
                    crate::ast::ResultAllocationModifier {
                        span: callable.span,
                    },
                ),
                capability: callable.capability,
                parameters_span: callable.span,
                parameters,
                return_type: Box::new(return_type),
                result_provenance: callable.result_provenance.clone(),
            }))
        }
        Type::Closure(closure) => Some(TypeExpr::Closure(closure.clone())),
        Type::I32 => Some(type_reference("i32", span)),
        Type::Primitive(name) => Some(type_reference(name, span)),
        Type::Named(name) => Some(type_reference(name, span)),
        Type::StrData => Some(type_reference("str", span)),
        Type::Str => Some(TypeExpr::Borrow(BorrowType {
            span,
            is_readwrite: false,
            inner: Box::new(type_reference("str", span)),
        })),
        Type::Error => Some(type_reference("error", span)),
        Type::Void => Some(type_reference("void", span)),
        Type::Never => Some(type_reference("never", span)),
        Type::ArrayData { element } => Some(TypeExpr::View(ViewType {
            span,
            is_readwrite: false,
            element: Box::new(type_to_type_expr_inner(
                element,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
        })),
        Type::View {
            is_readwrite,
            element,
        } => Some(TypeExpr::Borrow(BorrowType {
            span,
            is_readwrite: *is_readwrite,
            inner: Box::new(TypeExpr::View(ViewType {
                span,
                is_readwrite: false,
                element: Box::new(type_to_type_expr_inner(
                    element,
                    span,
                    free_type_parameters.as_deref_mut(),
                )?),
            })),
        })),
        Type::Array { element, length } => Some(TypeExpr::Array(ArrayType {
            span,
            element: Box::new(type_to_type_expr_inner(
                element,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
            length: ArrayLength {
                span,
                value: length.clone(),
            },
        })),
        Type::Pointer(inner) => Some(TypeExpr::Pointer(PointerType {
            span,
            inner: Box::new(type_to_type_expr_inner(
                inner,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
        })),
        Type::Borrow {
            is_readwrite,
            inner,
        } => Some(TypeExpr::Borrow(BorrowType {
            span,
            is_readwrite: *is_readwrite,
            inner: Box::new(type_to_type_expr_inner(
                inner,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
        })),
        Type::Optional(inner) => Some(TypeExpr::Optional(OptionalType {
            span,
            inner: Box::new(type_to_type_expr_inner(
                inner,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
        })),
        Type::Fallible { success, error } => Some(TypeExpr::Fallible(FallibleType {
            span,
            success: Box::new(type_to_type_expr_inner(
                success,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
            error: Box::new(type_to_type_expr_inner(
                error,
                span,
                free_type_parameters.as_deref_mut(),
            )?),
        })),
        Type::Generic { name, arguments } => Some(TypeExpr::Generic(GenericType {
            span,
            name: name.clone(),
            name_span: span,
            arguments: arguments
                .iter()
                .map(|argument| {
                    type_to_type_expr_inner(argument, span, free_type_parameters.as_deref_mut())
                })
                .collect::<Option<Vec<_>>>()?,
        })),
        Type::Parameter(name) => {
            let free_type_parameters = free_type_parameters?;
            free_type_parameters.insert(name.clone());
            Some(type_reference(name, span))
        }
        Type::None | Type::Unresolved(_) | Type::Unknown => None,
    }
}

pub(super) fn type_reference(name: impl Into<String>, span: ByteSpan) -> TypeExpr {
    TypeExpr::Reference(TypeReference {
        span,
        name: name.into(),
    })
}

pub(super) fn scalar_view_kind(ty: &Type) -> Option<TypecheckScalarViewKind> {
    match ty {
        Type::I32 => Some(TypecheckScalarViewKind::I32),
        Type::Primitive(name) if name == "u8" => Some(TypecheckScalarViewKind::U8),
        Type::Primitive(name) if name == "usize" => Some(TypecheckScalarViewKind::Usize),
        Type::Primitive(name) if name == "bool" => Some(TypecheckScalarViewKind::Bool),
        Type::Str => Some(TypecheckScalarViewKind::Str),
        Type::View { element, .. } => {
            Some(TypecheckScalarViewKind::Slice(slice_element_kind(element)))
        }
        _ => None,
    }
}

pub(super) fn slice_element_kind(element: &Type) -> TypecheckSliceElementKind {
    match element {
        Type::I32 => TypecheckSliceElementKind::I32,
        Type::Primitive(name) if name == "u8" => TypecheckSliceElementKind::U8,
        Type::Primitive(name) if name == "usize" => TypecheckSliceElementKind::Usize,
        Type::Primitive(name) if name == "bool" => TypecheckSliceElementKind::Bool,
        Type::Str => TypecheckSliceElementKind::Str,
        _ => TypecheckSliceElementKind::Other,
    }
}
