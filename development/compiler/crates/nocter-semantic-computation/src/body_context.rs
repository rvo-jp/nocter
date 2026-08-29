use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use crate::{
    CurrentSourceScopeInput, DeclarationQuery, DeclarationQueryOutcome, ProgramPreparationOutcome,
    SemanticScopeKey,
};

pub(crate) struct BodySemanticContextQuery;

struct BodySemanticContext {
    unit: Arc<nocter_discovery::DiscoveredUnit>,
    projection: nocter_declaration_lowering::CurrentDeclarationProjection,
    checking: nocter_checking::ProgramBodyCheckingContext,
}

pub(crate) struct BodySemanticContextProduct {
    context: Option<BodySemanticContext>,
    fingerprint: Fingerprint,
}

impl BodySemanticContextProduct {
    /// Resolves only after the exact body input has been demanded by the calling body query.
    pub(crate) fn resolve_names(
        &self,
        _exact_body: &crate::BodySourceValue,
        identity: &nocter_declaration_lowering::ReusableBodyIdentity,
    ) -> Option<nocter_checking::ReusableBodyNameQueryOutcome> {
        let context = self.context.as_ref()?;
        let input = context.unit.compile_input().ok()?;
        context
            .checking
            .resolve_names(
                &input,
                context.projection.frontend_bindings(),
                identity.body(),
            )
            .ok()
    }

    /// Types only after the exact body and its source-neutral lexical result have been demanded.
    pub(crate) fn check_body(
        &self,
        _exact_body: &crate::BodySourceValue,
        names: &nocter_checking::ReusableBodyNames,
    ) -> Option<nocter_checking::ReusableBodyQueryOutcome> {
        let context = self.context.as_ref()?;
        let input = context.unit.compile_input().ok()?;
        context
            .checking
            .check(&input, context.projection.frontend_bindings(), names)
            .ok()
    }

    pub(crate) fn finalize(
        &self,
        body_names: &crate::BodyNameSet,
        typed_bodies: &crate::TypedBodySet,
    ) -> Option<nocter_checking::QueriedProgramFinalizationOutcome> {
        let context = self.context.as_ref()?;
        let input = context.unit.compile_input().ok()?;
        let names = body_names
            .entries()
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>();
        let name_rejections = body_names
            .rejections()
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>();
        let bodies = typed_bodies
            .entries()
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>();
        let body_rejections = typed_bodies
            .rejections()
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>();
        context
            .checking
            .finalize(
                &input,
                context.projection.frontend_bindings(),
                &names,
                &name_rejections,
                &bodies,
                &body_rejections,
            )
            .ok()
    }
}

impl QueryValue for BodySemanticContextProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for BodySemanticContextQuery {
    type Key = SemanticScopeKey;
    type Value = BodySemanticContextProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let declarations = database.query::<DeclarationQuery>(key.clone())?;
        let declaration_fingerprint = declarations.fingerprint();
        let preparation = crate::prepared_program(database, key.clone())?;
        let current = database.input::<CurrentSourceScopeInput>(key)?;
        let context = match (declarations.outcome(), preparation.outcome()) {
            (
                DeclarationQueryOutcome::Accepted(declarations),
                ProgramPreparationOutcome::Prepared(prepared),
            ) => current.unit.compile_input().ok().and_then(|input| {
                let projection = declarations.materialize_projection(&input).ok()?;
                let checking = nocter_checking::ProgramBodyCheckingContext::new(
                    prepared,
                    projection.checking_symbols().spellings(),
                    projection.frontend_bindings(),
                    projection.source_index().clone(),
                );
                Some(BodySemanticContext {
                    unit: Arc::clone(&current.unit),
                    projection,
                    checking,
                })
            }),
            _ => None,
        };
        let fingerprint = if context.is_some() {
            declaration_fingerprint
        } else {
            current.fingerprint
        };
        Ok(BodySemanticContextProduct {
            context,
            fingerprint,
        })
    }
}
