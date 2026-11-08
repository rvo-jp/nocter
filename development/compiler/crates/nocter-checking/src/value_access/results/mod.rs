//! One closure of callable result producers, followed by identity-only body projection.
mod producers;
mod projection;
mod solve;
mod typing;

use super::{Failure, Query, accepted, flow};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema, Program};
use nocter_model::{AccessCallableId, AccessExpressionId, TypeId, ValueFunctionOrigin};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug)]
pub(super) struct Equality {
    left: TypeId,
    right: TypeId,
    /// The boundary or conditional operation emits this reserved identity immediately next.
    origin: AccessExpressionId,
}
#[derive(Clone, Debug, Default)]
pub(super) struct Evidence {
    producers: BTreeMap<AccessCallableId, FunctionWitness>,
    bodies: BTreeMap<AccessCallableId, Arc<[u8]>>,
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
        let syntax = context.syntax();
        for (&id, bytes) in &self.bodies {
            let body = context
                .callable_source(id)
                .ok_or(Failure::ChangedBody)?
                .body();
            let range = syntax
                .tree()
                .node(body)
                .ok_or(Failure::InconsistentSyntax)?
                .range();
            if syntax
                .text_at(range)
                .ok_or(Failure::InconsistentSyntax)?
                .as_bytes()
                != bytes.as_ref()
            {
                return Err(Failure::ChangedBody);
            }
        }
        Ok(Verified {
            evidence: self,
            context,
        })
    }
    fn witness(&self, witness: FunctionWitness) -> Result<FunctionWitness, Failure> {
        let ValueFunctionOrigin::Result(id) = witness.origin else {
            return Ok(witness);
        };
        let producer = self.producers.get(&id).ok_or(Failure::InconsistentSyntax)?;
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
                if matches!(witness.origin, ValueFunctionOrigin::Result(_)) =>
            {
                program
                    .function_data(self.witness(witness)?)
                    .ok_or(Failure::InconsistentSyntax)
            }
            _ => Ok(data),
        }
    }
}
pub(super) fn is_result(program: &Program, data: TypeId) -> bool {
    matches!(
        program.data_schema(data),
        Some(PayloadSchema::Function(FunctionWitness {
            origin: ValueFunctionOrigin::Result(_),
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
        projection::project(context, query, &evidence)
            .map_err(|failure| accepted::Failure::Body { callable, failure })?;
        query.results_closed = true;
        query.result_evidence = Some(evidence.clone());
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
        callable: query.callable,
        failure: super::diagnostics::query_rejection(
            context,
            query,
            origin,
            super::Rule::TypeMismatch,
            message,
        ),
    }
}
