//! Canonical body type replay and one complete typed-arena data projection.
use super::{
    Failure, Query, accepted,
    ir::{Expression, ValueType},
};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::Program;
use nocter_model::{AccessCodeId, AccessExpressionId, TypeId, ValueAccess};
use std::{collections::BTreeMap, sync::Arc};

pub(super) fn canonicalize(
    context: &DeclarationContext<'_>,
    queries: &mut BTreeMap<AccessCodeId, Query>,
) -> Result<Arc<Program>, accepted::Failure> {
    let canonical = context
        .program()
        .canonical_body_types(
            queries
                .iter()
                .map(|(&id, query)| (id, query.program()))
                .collect(),
        )
        .map_err(|_| {
            accepted::Failure::Integrity(super::flow::IntegrityError::InconsistentTypedBody)
        })?;
    for (&callable, query) in &mut *queries {
        project(
            query,
            |data| {
                canonical
                    .project(callable, data)
                    .ok_or(Failure::InconsistentSyntax)
            },
            |_, _, _| Ok(()),
        )
        .map_err(|failure| accepted::Failure::Body { callable, failure })?;
    }
    let program = Arc::new(canonical.finish());
    for query in queries.values_mut() {
        query.program = program.clone();
        for nested in query.closures.values_mut() {
            Arc::make_mut(&mut nested.typed).program = program.clone();
        }
    }
    Ok(program)
}

/// All data-bearing slots are rewritten together. Source and semantic operation identities stay fixed.
pub(super) fn project(
    query: &mut Query,
    mut data: impl FnMut(TypeId) -> Result<TypeId, Failure>,
    mut expression: impl FnMut(&Query, AccessExpressionId, &mut Expression) -> Result<(), Failure>,
) -> Result<(), Failure> {
    visit(query, &mut data, &mut expression)
}

fn visit(
    query: &mut Query,
    data: &mut dyn FnMut(TypeId) -> Result<TypeId, Failure>,
    expression: &mut dyn FnMut(&Query, AccessExpressionId, &mut Expression) -> Result<(), Failure>,
) -> Result<(), Failure> {
    query.body.locals = query.body.locals.try_map(|_, local| {
        let mut local = local.clone();
        local.data = data(local.data)?;
        Ok::<_, Failure>(local)
    })?;
    query.body.places = query.body.places.try_map(|_, place| {
        let mut place = place.clone();
        place.data = data(place.data)?;
        Ok::<_, Failure>(place)
    })?;
    query.body.expressions = query.body.expressions.try_map(|id, original| {
        let mut value = original.clone();
        value.ty = value_type(value.ty, &mut *data)?;
        expression(query, id, &mut value)?;
        Ok::<_, Failure>(value)
    })?;
    query.body.blocks = query.body.blocks.try_map(|_, block| {
        let mut block = block.clone();
        block.ty = value_type(block.ty, &mut *data)?;
        Ok::<_, Failure>(block)
    })?;
    query.result_data = data(query.result_data)?;
    for equality in &mut query.equalities {
        equality.left = data(equality.left)?;
        equality.right = data(equality.right)?;
    }
    for nested in query.closures.values_mut() {
        visit(Arc::make_mut(&mut nested.typed), data, expression)?;
    }
    Ok(())
}
fn value_type(
    ty: ValueType,
    mut data: impl FnMut(TypeId) -> Result<TypeId, Failure>,
) -> Result<ValueType, Failure> {
    let Some(access) = ty.access() else {
        return Ok(ty);
    };
    Ok(
        ValueType::known(ValueAccess::new(data(access.data())?, access.mode()))
            .with_continuation(ty.may_complete()),
    )
}
