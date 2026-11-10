//! One closure of callable result producers, followed by identity-only body projection.
mod producers;
mod projection;
mod relations;
mod solve;
mod typing;
mod unproduced;

use super::{Failure, Query, accepted, flow};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema, Program};
use nocter_model::{
    AccessCallableId, AccessExpressionId, TypeId, ValueApplication, ValueFunctionOrigin,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug)]
pub(super) struct Equality {
    pub(super) left: TypeId,
    pub(super) right: TypeId,
    /// The boundary or conditional operation emits this reserved identity immediately next.
    origin: AccessExpressionId,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Evidence {
    producers: BTreeMap<AccessCallableId, FunctionWitness>,
    applications: BTreeMap<ValueApplication, ApplicationEvidence>,
    bodies: nocter_declaration_lowering::access_declarations::BodySources,
}
#[derive(Clone, Copy, Debug)]
enum ApplicationEvidence {
    Witness(FunctionWitness),
    /// The structural contract is known, but no reachable invocation produces this value.
    Unproduced,
}
pub(super) struct Verified<'a, 'source> {
    pub(super) evidence: &'a Evidence,
    pub(super) context: &'a DeclarationContext<'source>,
}
impl Evidence {
    pub(super) fn producer(&self, callable: AccessCallableId) -> Option<FunctionWitness> {
        self.producers.get(&callable).copied()
    }
    pub(super) fn verify<'a, 'source>(
        &'a self,
        context: &'a DeclarationContext<'source>,
    ) -> Result<Verified<'a, 'source>, Failure> {
        self.bodies.verify(context)?;
        Ok(Verified {
            evidence: self,
            context,
        })
    }
    fn selected(&self, origin: ValueFunctionOrigin) -> Option<FunctionWitness> {
        match origin {
            ValueFunctionOrigin::Result(id) => self.producers.get(&id).copied(),
            ValueFunctionOrigin::Application(application) => {
                match self.applications.get(&application)? {
                    ApplicationEvidence::Witness(witness) => Some(*witness),
                    ApplicationEvidence::Unproduced => None,
                }
            }
            _ => None,
        }
    }
    fn origin(&self, origin: ValueFunctionOrigin) -> Result<ValueFunctionOrigin, Failure> {
        if self.unproduced_origin(origin) {
            return Ok(origin);
        }
        match origin {
            ValueFunctionOrigin::Declaration(_)
            | ValueFunctionOrigin::Closure(_)
            | ValueFunctionOrigin::Parameter(_) => Ok(origin),
            _ => self
                .selected(origin)
                .map(|w| w.origin)
                .ok_or(Failure::InconsistentSyntax),
        }
    }
    fn witness(&self, witness: FunctionWitness) -> Result<FunctionWitness, Failure> {
        if self.unproduced_origin(witness.origin) {
            return Ok(witness);
        }
        let Some(producer) = self.selected(witness.origin) else {
            return match witness.origin {
                ValueFunctionOrigin::Declaration(_)
                | ValueFunctionOrigin::Closure(_)
                | ValueFunctionOrigin::Parameter(_) => Ok(witness),
                _ => Err(Failure::InconsistentSyntax),
            };
        };
        if !producer.invocation.can_supply(witness.invocation) {
            return Err(Failure::InconsistentSyntax);
        }
        Ok(FunctionWitness {
            origin: producer.origin,
            invocation: witness.invocation,
        })
    }
    fn data(&self, program: &Program, data: TypeId) -> Result<TypeId, Failure> {
        match program
            .data_schema(data)
            .ok_or(Failure::InconsistentSyntax)?
        {
            PayloadSchema::Function(witness)
                if matches!(
                    witness.origin,
                    ValueFunctionOrigin::Result(_) | ValueFunctionOrigin::Application(_)
                ) =>
            {
                program
                    .function_data(self.witness(witness)?)
                    .ok_or(Failure::InconsistentSyntax)
            }
            _ => Ok(data),
        }
    }
    fn unproduced_origin(&self, origin: ValueFunctionOrigin) -> bool {
        matches!(origin, ValueFunctionOrigin::Application(application)
            if matches!(self.applications.get(&application), Some(ApplicationEvidence::Unproduced)))
    }
    pub(super) fn unproduced(&self, program: &Program, data: TypeId) -> bool {
        matches!(program.data_schema(data), Some(PayloadSchema::Function(witness))
            if self.unproduced_origin(witness.origin))
    }
}
pub(super) use unproduced::validate;
pub(super) fn is_result(program: &Program, data: TypeId) -> bool {
    matches!(
        program.data_schema(data),
        Some(PayloadSchema::Function(FunctionWitness {
            origin: ValueFunctionOrigin::Result(_) | ValueFunctionOrigin::Application(_),
            ..
        }))
    )
}

pub(super) fn close(
    context: &DeclarationContext<'_>,
    controls: &mut BTreeMap<AccessCallableId, flow::Control>,
) -> Result<Arc<Evidence>, accepted::Failure> {
    let evidence = Arc::new(solve::solve(context, controls)?);
    for (&callable, control) in controls {
        if control.typed.results_closed() {
            continue;
        }
        let query = Arc::make_mut(&mut control.typed);
        query.result_evidence = Some(evidence.clone());
        projection::project(context, query, &evidence)
            .map_err(|failure| accepted::Failure::Body { callable, failure })?;
        query.results_closed = true;
    }
    Ok(evidence)
}

fn reject(
    context: &DeclarationContext<'_>,
    query: &Query,
    origin: Option<AccessExpressionId>,
    message: &str,
) -> accepted::Failure {
    accepted::Failure::Body {
        callable: query.source_owner(),
        failure: super::diagnostics::query_rejection(
            context,
            query,
            origin,
            super::Rule::TypeMismatch,
            message,
        ),
    }
}
