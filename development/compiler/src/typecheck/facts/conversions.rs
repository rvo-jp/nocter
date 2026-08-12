//! Immutable conversion facts shared by source collection and generic specialization.

use super::*;

pub(crate) fn typecheck_conversion_plan(
    expression_span: ByteSpan,
    source_span: ByteSpan,
    operator_span: Option<ByteSpan>,
    selected: crate::typecheck::conversions::SelectedConversion,
) -> Option<TypecheckConversionPlan> {
    let mut free_type_parameters = HashSet::new();
    let source_ty = type_to_type_expr_allowing_parameters(
        &selected.source_type,
        expression_span,
        &mut free_type_parameters,
    )?;
    let target_ty = type_to_type_expr_allowing_parameters(
        &selected.target_type,
        expression_span,
        &mut free_type_parameters,
    )?;
    let kind = match selected.kind {
        crate::typecheck::conversions::SelectedConversionKind::Exact => return None,
        crate::typecheck::conversions::SelectedConversionKind::LosslessInteger => {
            TypecheckConversionKind::LosslessInteger
        }
        crate::typecheck::conversions::SelectedConversionKind::CapabilityWeakening => {
            TypecheckConversionKind::CapabilityWeakening
        }
        crate::typecheck::conversions::SelectedConversionKind::BorrowCoercion(coercion) => {
            let self_ty = type_to_type_expr_allowing_parameters(
                &coercion.source_type,
                expression_span,
                &mut free_type_parameters,
            )?;
            let substitutions = coercion
                .substitutions
                .iter()
                .map(|(name, ty)| {
                    type_to_type_expr_allowing_parameters(
                        ty,
                        expression_span,
                        &mut free_type_parameters,
                    )
                    .map(|ty| (name.clone(), ty))
                })
                .collect::<Option<HashMap<_, _>>>()?;
            TypecheckConversionKind::BorrowCoercion(TypecheckCoercionPlan {
                def_id: coercion.def_id,
                declaration_span: coercion.declaration_span,
                focus_span: coercion.focus_span,
                receiver_mode: coercion.receiver_mode,
                source_is_readwrite: coercion.source_is_readwrite,
                target_name: format!(
                    "{}.__nocter$coerce${}",
                    canonical_type_expr(&self_ty),
                    coercion.focus_span.start
                ),
                self_ty,
                target_ty: target_ty.clone(),
                substitutions,
                has_explicit_result_provenance: coercion.has_explicit_result_provenance,
                requirement_span: coercion.requirement_span,
                free_type_parameters: free_type_parameters.clone(),
            })
        }
    };
    Some(TypecheckConversionPlan {
        expression_span,
        source_span,
        operator_span,
        source_ty,
        target_ty,
        kind,
    })
}
