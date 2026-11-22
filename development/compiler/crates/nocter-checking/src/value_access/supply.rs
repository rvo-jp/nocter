//! One unmarked snapshot decision, closed before initialization; explicit transfers are frozen earlier.
use super::{
    Failure, Query, Rule,
    ir::{ArgumentAction, Boundary, BoundaryAction, Expression, ExpressionKind, Referent},
};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_declarations::value_access::{PayloadReadiness, Program};
use nocter_model::{AccessExpressionId, TypeId, ValueAccess, ValueAccessMode as Mode};
use nocter_syntax::{NodeId, SyntaxOrigin};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PlaceSupply {
    Copy,
    Pending,
    MissingCopy,
}

/// A structural result is pending, never provisional Copy. All consumers use this same choice.
pub(super) fn select(program: &Program, data: TypeId) -> Result<PlaceSupply, Failure> {
    let copyable = program
        .data_copyable(data)
        .ok_or(Failure::InconsistentSyntax)?;
    Ok(
        if program.payload_readiness(data) == PayloadReadiness::Pending {
            PlaceSupply::Pending
        } else if copyable {
            PlaceSupply::Copy
        } else {
            PlaceSupply::MissingCopy
        },
    )
}

impl super::checker::Checker<'_, '_> {
    /// Validate access before asking for payload properties, then freeze the single supply action.
    pub(super) fn value_supply_action(
        &self,
        node: NodeId,
        source: AccessExpressionId,
        boundary: Boundary,
        actual: ValueAccess,
    ) -> Result<BoundaryAction, Failure> {
        let expected = boundary.ty();
        if expected.mode() == Mode::Own && !self.ty(source).may_complete() {
            return Ok(BoundaryAction::Unproduced);
        }
        let place = self.expression_referent(source);
        match expected.mode() {
            Mode::Read if place.is_none() => return self.reject(Rule::TypeMismatch, node),
            Mode::Update => {
                let Some(referent) = place else {
                    return self.reject(Rule::InvalidUpdate, node);
                };
                self.require_writable_referent(referent, node, Rule::InvalidUpdate)?;
            }
            Mode::Own if place.is_none() && actual.mode() != Mode::Own => {
                return Err(Failure::InconsistentSyntax);
            }
            Mode::Read | Mode::Own => {}
        }
        // A local owning slot can hold the checked unstarted activation. Other boundaries
        // still require their own origin/transport evidence, independently of the descriptor.
        if !(matches!(boundary, Boundary::Binding(_))
            && expected.mode() == Mode::Own
            && matches!(
                self.program().data_schema(actual.data()),
                Some(nocter_declarations::value_access::PayloadSchema::Future(_))
            ))
        {
            super::checker::payload_requirements::data(
                self.program(),
                self.syntax(),
                actual.data(),
                SyntaxOrigin::Node(node),
            )?;
        }
        match expected.mode() {
            Mode::Read => Ok(BoundaryAction::ReadPlace),
            Mode::Update => Ok(BoundaryAction::DelegateUpdate),
            Mode::Own => {
                let Some(local) = place else {
                    return Ok(BoundaryAction::TakeValue);
                };
                match select(self.program(), actual.data())? {
                    PlaceSupply::Pending => Ok(BoundaryAction::PendingValue),
                    PlaceSupply::Copy => Ok(BoundaryAction::CopyPayload),
                    PlaceSupply::MissingCopy => {
                        if let Some(place) = local.place() {
                            return self.reject_local(
                                Rule::MissingTransfer,
                                node,
                                self.selected_place(place).local,
                            );
                        }
                        self.reject(Rule::MissingTransfer, node)
                    }
                }
            }
        }
    }
}

/// Resolve this expression's pending plans during the sole identity-projection traversal.
/// Known generic formal transfers remain source-selected transfers.
pub(super) fn resolve(
    context: &DeclarationContext<'_>,
    query: &Query,
    id: AccessExpressionId,
    expression: &mut Expression,
    project: &dyn Fn(TypeId) -> Result<TypeId, Failure>,
) -> Result<(), Failure> {
    match &mut expression.kind {
        ExpressionKind::Boundary {
            source,
            action: action @ BoundaryAction::PendingValue,
        } => {
            let place = query
                .body
                .expressions
                .get(*source)
                .and_then(|expression| expression.referent(*source))
                .ok_or(Failure::InconsistentSyntax)?;
            if unproduced(query, place, project)? {
                *action = BoundaryAction::Unproduced;
                return Ok(());
            }
            match resolved(context, query, id, place, project)? {
                PlaceSupply::Copy => *action = BoundaryAction::CopyPayload,
                PlaceSupply::Pending | PlaceSupply::MissingCopy => unreachable!("resolved above"),
            }
        }
        ExpressionKind::Call { arguments, .. } => {
            for argument in arguments {
                if let ArgumentAction::PendingValue(place) = argument.action {
                    if unproduced(query, place, project)? {
                        argument.action = ArgumentAction::Unproduced;
                        continue;
                    }
                    argument.action =
                        match resolved(context, query, argument.value, place, project)? {
                            PlaceSupply::Copy => ArgumentAction::CopyPlace(place),
                            PlaceSupply::Pending | PlaceSupply::MissingCopy => {
                                unreachable!("resolved above")
                            }
                        };
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn unproduced(
    query: &Query,
    place: Referent,
    project: &dyn Fn(TypeId) -> Result<TypeId, Failure>,
) -> Result<bool, Failure> {
    let data = query
        .body
        .referent_data(place)
        .ok_or(Failure::InconsistentSyntax)?;
    let data = project(data)?;
    Ok(query
        .result_evidence
        .as_ref()
        .is_some_and(|e| e.unproduced(query.program(), data)))
}

fn resolved(
    context: &DeclarationContext<'_>,
    query: &Query,
    origin: AccessExpressionId,
    place: Referent,
    project: &dyn Fn(TypeId) -> Result<TypeId, Failure>,
) -> Result<PlaceSupply, Failure> {
    let data = query
        .body
        .referent_data(place)
        .ok_or(Failure::InconsistentSyntax)?;
    match select(query.program(), project(data)?)? {
        PlaceSupply::Pending => Err(Failure::InconsistentSyntax),
        PlaceSupply::MissingCopy => Err(super::diagnostics::query_rejection(
            context,
            query,
            Some(origin),
            Rule::MissingTransfer,
            "this unmarked value has no Copy evidence; transfer from an eligible owner requires postfix ^",
        )),
        PlaceSupply::Copy => Ok(PlaceSupply::Copy),
    }
}
