//! Structural MIR verification. Validation errors are compiler invariant
//! failures, not alternate source-language diagnostics.

use super::ids::{BasicBlockId, LocalId, ProjectionPathId};
use super::locals::{LocalOrigin, LocalStorage, OwnershipKind, ScalarType, ValueRepresentation};
use super::model::{
    Body, CallContinuation, Operand, ProjectionElement, ProjectionPath, Statement, Terminator,
};
use crate::semantic::TyId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ValidationError {
    Initialization(super::initialization::InitializationError),
    DropObligation(super::drop_obligations::DropObligationError),
    Loan(super::loans::LoanError),
    InvalidProjectionIdentity(ProjectionPathId),
    MissingProjectionBase {
        projection: ProjectionPathId,
        local: LocalId,
    },
    InvalidProjectionParent {
        projection: ProjectionPathId,
        parent: ProjectionPathId,
    },
    ProjectionBaseMismatch {
        projection: ProjectionPathId,
        expected: LocalId,
        actual: LocalId,
    },
    ProjectionOfScalar {
        projection: ProjectionPathId,
    },
    InvalidProjectionIndex {
        projection: ProjectionPathId,
    },
    MissingPlaceProjection {
        block: BasicBlockId,
        location: OperandLocation,
        projection: ProjectionPathId,
    },
    MissingReturnLocal(LocalId),
    MissingRootScope(super::ScopeId),
    InvalidRootScope(super::ScopeId),
    MissingLocalScope {
        local: LocalId,
        scope: super::ScopeId,
    },
    InvalidScopeParent {
        scope: super::ScopeId,
        parent: super::ScopeId,
    },
    InvalidLocalContract(LocalId),
    DuplicateParameterStorage {
        first: LocalId,
        duplicate: LocalId,
        index: usize,
    },
    MissingEntryBlock(BasicBlockId),
    MissingBlockScope {
        block: BasicBlockId,
        scope: super::ScopeId,
    },
    MissingAssignmentLocal {
        block: BasicBlockId,
        statement: usize,
        local: LocalId,
    },
    AssignmentTypeMismatch {
        block: BasicBlockId,
        statement: usize,
        destination: TyId,
        value: TyId,
    },
    MissingOperandLocal {
        block: BasicBlockId,
        location: OperandLocation,
        local: LocalId,
    },
    OperandTypeMismatch {
        block: BasicBlockId,
        location: OperandLocation,
        expected: TyId,
        actual: TyId,
    },
    OperandScalarMismatch {
        block: BasicBlockId,
        location: OperandLocation,
        expected: ScalarType,
        actual: ScalarType,
    },
    AssignmentScalarMismatch {
        block: BasicBlockId,
        statement: usize,
        expected: ScalarType,
        actual: ScalarType,
    },
    OperandRepresentationMismatch {
        block: BasicBlockId,
        location: OperandLocation,
        expected: ValueRepresentation,
        actual: ValueRepresentation,
    },
    InvalidOperandOwnership {
        block: BasicBlockId,
        location: OperandLocation,
        local: LocalId,
        operand: OperandOwnership,
        ownership: OwnershipKind,
    },
    AssignmentRequiresScalar {
        block: BasicBlockId,
        statement: usize,
        actual: ValueRepresentation,
    },
    InvalidUnaryOperation {
        block: BasicBlockId,
        statement: usize,
        operator: super::UnaryOperator,
        scalar: ScalarType,
    },
    MissingTarget {
        block: BasicBlockId,
        target: BasicBlockId,
    },
    InvalidScopeTransition {
        block: BasicBlockId,
        target: BasicBlockId,
    },
    MissingCallDestination {
        block: BasicBlockId,
        local: LocalId,
    },
    NonBooleanCondition {
        block: BasicBlockId,
        actual: ValueRepresentation,
    },
    PropagationFromPlainBody {
        block: BasicBlockId,
    },
    InvalidLoopCondition {
        header: BasicBlockId,
        condition: BasicBlockId,
    },
    DuplicateLoopHeader {
        header: BasicBlockId,
    },
    InvalidLoopHeaderPath {
        header: BasicBlockId,
        condition: BasicBlockId,
    },
    InvalidLoopContinuePath {
        header: BasicBlockId,
        continue_target: BasicBlockId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperandLocation {
    Statement(usize),
    CallArgument(usize),
    Drop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperandOwnership {
    Copy,
    Move,
}

pub(crate) fn validate(body: &Body) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    if body.locals.get(body.return_local.index()).is_none() {
        errors.push(ValidationError::MissingReturnLocal(body.return_local));
    }
    validate_scopes(body, &mut errors);
    validate_local_contracts(body, &mut errors);
    validate_projection_paths(body, &mut errors);
    if body.blocks.get(body.entry.index()).is_none() {
        errors.push(ValidationError::MissingEntryBlock(body.entry));
    }
    let mut loop_headers = std::collections::HashSet::new();
    for region in &body.loop_regions {
        if !loop_headers.insert(region.header) {
            errors.push(ValidationError::DuplicateLoopHeader {
                header: region.header,
            });
        }
        for target in [
            region.header,
            region.condition,
            region.body,
            region.continue_target,
            region.exit,
        ] {
            if body.blocks.get(target.index()).is_none() {
                errors.push(ValidationError::MissingTarget {
                    block: region.header,
                    target,
                });
            }
        }
        if !matches!(
            body.blocks.get(region.condition.index()).map(|block| &block.terminator),
            Some(Terminator::Switch {
                then_target,
                else_target,
                ..
            }) if *then_target == region.body && *else_target == region.exit
        ) {
            errors.push(ValidationError::InvalidLoopCondition {
                header: region.header,
                condition: region.condition,
            });
        }
        if !linear_path_reaches(body, region.header, region.condition) {
            errors.push(ValidationError::InvalidLoopHeaderPath {
                header: region.header,
                condition: region.condition,
            });
        }
        if region.continue_target != region.header
            && !linear_path_reaches(body, region.continue_target, region.header)
        {
            errors.push(ValidationError::InvalidLoopContinuePath {
                header: region.header,
                continue_target: region.continue_target,
            });
        }
    }

    for (block_index, block) in body.blocks.iter().enumerate() {
        let block_id = BasicBlockId::from_index(block_index);
        if body.scopes.get(block.scope.index()).is_none() {
            errors.push(ValidationError::MissingBlockScope {
                block: block_id,
                scope: block.scope,
            });
        }
        for (statement_index, statement) in block.statements.iter().enumerate() {
            if !matches!(statement, Statement::Assign { .. }) {
                continue;
            }
            let Statement::Assign {
                destination, value, ..
            } = statement
            else {
                unreachable!("non-assignment statements were handled above")
            };
            let Some(destination_local) = body.locals.get(destination.local.index()) else {
                errors.push(ValidationError::MissingAssignmentLocal {
                    block: block_id,
                    statement: statement_index,
                    local: destination.local,
                });
                continue;
            };
            let value_ty = match value {
                super::model::Rvalue::Use(operand) => {
                    let ty = operand_type(
                        body,
                        block_id,
                        OperandLocation::Statement(statement_index),
                        operand,
                        &mut errors,
                    );
                    validate_operand_representation(
                        body,
                        block_id,
                        OperandLocation::Statement(statement_index),
                        operand,
                        destination_local.representation,
                        &mut errors,
                    );
                    validate_operand_ownership(
                        body,
                        block_id,
                        OperandLocation::Statement(statement_index),
                        operand,
                        &mut errors,
                    );
                    ty
                }
                super::model::Rvalue::Unary {
                    operator,
                    operand,
                    ty,
                } => {
                    if let Some(destination_scalar) = destination_local.scalar_type() {
                        validate_operand(
                            body,
                            block_id,
                            OperandLocation::Statement(statement_index),
                            operand,
                            *ty,
                            destination_scalar,
                            &mut errors,
                        );
                        let valid = match operator {
                            super::UnaryOperator::Negate => match destination_scalar {
                                ScalarType::I32 => true,
                                ScalarType::Integer(kind) => kind.is_signed(),
                                ScalarType::Usize | ScalarType::Bool => false,
                            },
                            super::UnaryOperator::LogicalNot => {
                                destination_scalar == ScalarType::Bool
                            }
                        };
                        if !valid {
                            errors.push(ValidationError::InvalidUnaryOperation {
                                block: block_id,
                                statement: statement_index,
                                operator: *operator,
                                scalar: destination_scalar,
                            });
                        }
                    } else {
                        errors.push(ValidationError::AssignmentRequiresScalar {
                            block: block_id,
                            statement: statement_index,
                            actual: destination_local.representation,
                        });
                    }
                    Some(*ty)
                }
                super::model::Rvalue::Binary {
                    left, right, ty, ..
                } => {
                    if let Some(destination_scalar) = destination_local.scalar_type() {
                        for operand in [left, right] {
                            validate_operand(
                                body,
                                block_id,
                                OperandLocation::Statement(statement_index),
                                operand,
                                *ty,
                                destination_scalar,
                                &mut errors,
                            );
                        }
                    } else {
                        errors.push(ValidationError::AssignmentRequiresScalar {
                            block: block_id,
                            statement: statement_index,
                            actual: destination_local.representation,
                        });
                    }
                    Some(*ty)
                }
                super::model::Rvalue::Compare {
                    left,
                    right,
                    operand_ty,
                    operand_scalar,
                    result_ty,
                    ..
                } => {
                    for operand in [left, right] {
                        validate_operand(
                            body,
                            block_id,
                            OperandLocation::Statement(statement_index),
                            operand,
                            *operand_ty,
                            *operand_scalar,
                            &mut errors,
                        );
                    }
                    match destination_local.representation {
                        ValueRepresentation::Scalar(actual) if actual != ScalarType::Bool => {
                            errors.push(ValidationError::AssignmentScalarMismatch {
                                block: block_id,
                                statement: statement_index,
                                expected: ScalarType::Bool,
                                actual,
                            });
                        }
                        ValueRepresentation::Borrow | ValueRepresentation::Aggregate => {
                            errors.push(ValidationError::AssignmentRequiresScalar {
                                block: block_id,
                                statement: statement_index,
                                actual: destination_local.representation,
                            });
                        }
                        ValueRepresentation::Scalar(_) => {}
                    }
                    Some(*result_ty)
                }
            };
            if value_ty.is_some_and(|value_ty| destination_local.ty != value_ty) {
                errors.push(ValidationError::AssignmentTypeMismatch {
                    block: block_id,
                    statement: statement_index,
                    destination: destination_local.ty,
                    value: value_ty.unwrap(),
                });
            }
        }

        match &block.terminator {
            Terminator::Goto { target } => {
                validate_target(body, block_id, *target, &mut errors);
            }
            Terminator::Switch {
                condition,
                then_target,
                else_target,
            } => {
                validate_target(body, block_id, *then_target, &mut errors);
                validate_target(body, block_id, *else_target, &mut errors);
                if let Some(actual) = operand_representation(body, condition)
                    && actual != ValueRepresentation::Scalar(ScalarType::Bool)
                {
                    errors.push(ValidationError::NonBooleanCondition {
                        block: block_id,
                        actual,
                    });
                }
            }
            Terminator::Call {
                arguments,
                continuation,
                ..
            } => {
                let destination = match continuation {
                    CallContinuation::Return {
                        destination,
                        target,
                    } => {
                        validate_target(body, block_id, *target, &mut errors);
                        Some(destination)
                    }
                    CallContinuation::Outcome {
                        destination,
                        success,
                        failure,
                    } => {
                        validate_target(body, block_id, *success, &mut errors);
                        validate_target(body, block_id, *failure, &mut errors);
                        Some(destination)
                    }
                    CallContinuation::Never => None,
                };
                if let Some(destination) = destination
                    && body.locals.get(destination.local.index()).is_none()
                {
                    errors.push(ValidationError::MissingCallDestination {
                        block: block_id,
                        local: destination.local,
                    });
                }
                for (index, argument) in arguments.iter().enumerate() {
                    let location = OperandLocation::CallArgument(index);
                    let actual =
                        operand_type(body, block_id, location, &argument.operand, &mut errors);
                    if actual.is_some_and(|actual| actual != argument.ty) {
                        errors.push(ValidationError::OperandTypeMismatch {
                            block: block_id,
                            location,
                            expected: argument.ty,
                            actual: actual.unwrap(),
                        });
                    }
                    validate_operand_representation(
                        body,
                        block_id,
                        location,
                        &argument.operand,
                        argument.representation,
                        &mut errors,
                    );
                    validate_operand_ownership(
                        body,
                        block_id,
                        location,
                        &argument.operand,
                        &mut errors,
                    );
                }
            }
            Terminator::Drop { place, target } => {
                validate_target(body, block_id, *target, &mut errors);
                operand_type(
                    body,
                    block_id,
                    OperandLocation::Drop,
                    &Operand::Move(*place),
                    &mut errors,
                );
            }
            Terminator::PropagateFailure if body.return_mode == super::ReturnMode::Plain => {
                errors.push(ValidationError::PropagationFromPlainBody { block: block_id });
            }
            Terminator::Trap | Terminator::PropagateFailure | Terminator::Return => {}
        }
    }

    errors.extend(
        super::initialization::validate(body)
            .into_iter()
            .map(ValidationError::Initialization),
    );
    errors.extend(
        super::drop_obligations::validate(body)
            .into_iter()
            .map(ValidationError::DropObligation),
    );
    errors.extend(
        super::loans::validate(body)
            .into_iter()
            .map(ValidationError::Loan),
    );

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_scopes(body: &Body, errors: &mut Vec<ValidationError>) {
    let Some(root) = body.scopes.get(body.root_scope.index()) else {
        errors.push(ValidationError::MissingRootScope(body.root_scope));
        return;
    };
    if root.parent.is_some() {
        errors.push(ValidationError::InvalidRootScope(body.root_scope));
    }
    for (index, scope) in body.scopes.iter().enumerate() {
        let id = super::ScopeId::from_index(index);
        if id != body.root_scope && scope.parent.is_none() {
            errors.push(ValidationError::InvalidRootScope(id));
        }
        if let Some(parent) = scope.parent
            && (parent.index() >= index || body.scopes.get(parent.index()).is_none())
        {
            errors.push(ValidationError::InvalidScopeParent { scope: id, parent });
        }
    }
    for (index, local) in body.locals.iter().enumerate() {
        if body.scopes.get(local.scope.index()).is_none() {
            errors.push(ValidationError::MissingLocalScope {
                local: LocalId::from_index(index),
                scope: local.scope,
            });
        }
    }
}

fn validate_local_contracts(body: &Body, errors: &mut Vec<ValidationError>) {
    let mut parameter_storage = std::collections::HashMap::new();
    let return_local_exists = body.locals.get(body.return_local.index()).is_some();
    for (index, local) in body.locals.iter().enumerate() {
        let id = LocalId::from_index(index);
        let storage_matches_origin = matches!(
            (local.storage, local.origin),
            (LocalStorage::Return, LocalOrigin::Return)
                | (LocalStorage::Parameter { .. }, LocalOrigin::Parameter(_))
                | (
                    LocalStorage::Local,
                    LocalOrigin::Binding(_) | LocalOrigin::Temporary(_) | LocalOrigin::Desugared(_)
                )
        );
        let return_storage_matches_identity = !return_local_exists
            || (id == body.return_local) == (local.storage == LocalStorage::Return);
        let scalar_ownership_is_trivial =
            !matches!(local.representation, ValueRepresentation::Scalar(_))
                || local.ownership == OwnershipKind::Copy;
        if !storage_matches_origin
            || !return_storage_matches_identity
            || !scalar_ownership_is_trivial
        {
            errors.push(ValidationError::InvalidLocalContract(id));
        }
        if let LocalStorage::Parameter { ordinal } = local.storage
            && let Some(first) = parameter_storage.insert(ordinal, id)
        {
            errors.push(ValidationError::DuplicateParameterStorage {
                first,
                duplicate: id,
                index: ordinal,
            });
        }
    }
}

fn validate_projection_paths(body: &Body, errors: &mut Vec<ValidationError>) {
    for (index, projection) in body.projections.iter().enumerate() {
        let id = ProjectionPathId::from_index(index);
        if projection.id != id {
            errors.push(ValidationError::InvalidProjectionIdentity(id));
        }
        let Some(base) = body.locals.get(projection.base.index()) else {
            errors.push(ValidationError::MissingProjectionBase {
                projection: id,
                local: projection.base,
            });
            continue;
        };
        let parent_representation = match projection.parent {
            Some(parent) if parent.index() >= index => {
                errors.push(ValidationError::InvalidProjectionParent {
                    projection: id,
                    parent,
                });
                continue;
            }
            Some(parent) => {
                let Some(parent_path) = body.projections.get(parent.index()) else {
                    errors.push(ValidationError::InvalidProjectionParent {
                        projection: id,
                        parent,
                    });
                    continue;
                };
                if parent_path.base != projection.base {
                    errors.push(ValidationError::ProjectionBaseMismatch {
                        projection: id,
                        expected: parent_path.base,
                        actual: projection.base,
                    });
                }
                parent_path.representation
            }
            None => base.representation,
        };
        if matches!(parent_representation, ValueRepresentation::Scalar(_)) {
            errors.push(ValidationError::ProjectionOfScalar { projection: id });
        }
        if let ProjectionElement::Index { index, .. } = &projection.element
            && operand_representation(body, index)
                != Some(ValueRepresentation::Scalar(ScalarType::Usize))
        {
            errors.push(ValidationError::InvalidProjectionIndex { projection: id });
        }
    }
}

fn linear_path_reaches(body: &Body, start: BasicBlockId, destination: BasicBlockId) -> bool {
    let mut current = start;
    let mut visited = std::collections::HashSet::new();
    loop {
        if current == destination {
            return true;
        }
        if !visited.insert(current) {
            return false;
        }
        let Some(block) = body.blocks.get(current.index()) else {
            return false;
        };
        current = match &block.terminator {
            Terminator::Goto { target } => *target,
            Terminator::Drop { target, .. } => *target,
            Terminator::Call {
                continuation: CallContinuation::Return { target, .. },
                ..
            } => *target,
            Terminator::Call {
                continuation:
                    CallContinuation::Outcome {
                        success, failure, ..
                    },
                ..
            } if body.blocks.get(failure.index()).is_some_and(|failure| {
                failure.statements.is_empty()
                    && matches!(
                        failure.terminator,
                        Terminator::Trap | Terminator::PropagateFailure
                    )
            }) =>
            {
                *success
            }
            _ => return false,
        };
    }
}

fn validate_operand(
    body: &Body,
    block: BasicBlockId,
    location: OperandLocation,
    operand: &Operand,
    expected_ty: TyId,
    expected_scalar: ScalarType,
    errors: &mut Vec<ValidationError>,
) -> Option<TyId> {
    let actual_ty = operand_type(body, block, location, operand, errors);
    if let Some(actual) = actual_ty
        && actual != expected_ty
    {
        errors.push(ValidationError::OperandTypeMismatch {
            block,
            location,
            expected: expected_ty,
            actual,
        });
    }
    validate_operand_scalar(body, block, location, operand, expected_scalar, errors);
    validate_operand_ownership(body, block, location, operand, errors);
    actual_ty
}

fn validate_operand_ownership(
    body: &Body,
    block: BasicBlockId,
    location: OperandLocation,
    operand: &Operand,
    errors: &mut Vec<ValidationError>,
) {
    let (place, operation) = match operand {
        Operand::Constant(_) => return,
        Operand::Copy(place) => (place, OperandOwnership::Copy),
        Operand::Move(place) => (place, OperandOwnership::Move),
    };
    let Some(local) = body.locals.get(place.local.index()) else {
        return;
    };
    let ownership = place_projection(body, *place).map_or(local.ownership, |path| path.ownership);
    let valid = match (operation, ownership) {
        (OperandOwnership::Copy, OwnershipKind::Copy)
        | (OperandOwnership::Copy, OwnershipKind::Borrowed { readwrite: false })
        | (OperandOwnership::Move, OwnershipKind::Move) => true,
        (OperandOwnership::Copy | OperandOwnership::Move, _) => false,
    };
    if !valid {
        errors.push(ValidationError::InvalidOperandOwnership {
            block,
            location,
            local: place.local,
            operand: operation,
            ownership,
        });
    }
}

fn validate_operand_scalar(
    body: &Body,
    block: BasicBlockId,
    location: OperandLocation,
    operand: &Operand,
    expected: ScalarType,
    errors: &mut Vec<ValidationError>,
) {
    match operand_representation(body, operand) {
        Some(ValueRepresentation::Scalar(actual)) if actual != expected => {
            errors.push(ValidationError::OperandScalarMismatch {
                block,
                location,
                expected,
                actual,
            });
        }
        Some(actual @ (ValueRepresentation::Borrow | ValueRepresentation::Aggregate)) => {
            errors.push(ValidationError::OperandRepresentationMismatch {
                block,
                location,
                expected: ValueRepresentation::Scalar(expected),
                actual,
            });
        }
        Some(ValueRepresentation::Scalar(_)) | None => {}
    }
}

fn validate_operand_representation(
    body: &Body,
    block: BasicBlockId,
    location: OperandLocation,
    operand: &Operand,
    expected: ValueRepresentation,
    errors: &mut Vec<ValidationError>,
) {
    if let Some(actual) = operand_representation(body, operand)
        && actual != expected
    {
        errors.push(ValidationError::OperandRepresentationMismatch {
            block,
            location,
            expected,
            actual,
        });
    }
}

fn validate_target(
    body: &Body,
    block: BasicBlockId,
    target: BasicBlockId,
    errors: &mut Vec<ValidationError>,
) {
    let Some(source) = body.blocks.get(block.index()) else {
        return;
    };
    let Some(target_block) = body.blocks.get(target.index()) else {
        errors.push(ValidationError::MissingTarget { block, target });
        return;
    };
    if body.scopes.get(source.scope.index()).is_some()
        && body.scopes.get(target_block.scope.index()).is_some()
        && super::scopes::exited_scopes(&body.scopes, source.scope, target_block.scope).is_none()
    {
        errors.push(ValidationError::InvalidScopeTransition { block, target });
    }
}

fn operand_representation(body: &Body, operand: &Operand) -> Option<ValueRepresentation> {
    match operand {
        Operand::Constant(constant) => Some(ValueRepresentation::Scalar(constant.scalar)),
        Operand::Copy(place) | Operand::Move(place) => place_projection(body, *place)
            .map(|projection| projection.representation)
            .or_else(|| {
                body.locals
                    .get(place.local.index())
                    .map(|local| local.representation)
            }),
    }
}

fn operand_type(
    body: &Body,
    block: BasicBlockId,
    location: OperandLocation,
    operand: &Operand,
    errors: &mut Vec<ValidationError>,
) -> Option<TyId> {
    match operand {
        Operand::Constant(constant) => Some(constant.ty),
        Operand::Copy(place) | Operand::Move(place) => match body.locals.get(place.local.index()) {
            Some(local) => {
                if let Some(projection) = place.projection {
                    match body.projections.get(projection.index()) {
                        Some(path) if path.base == place.local => Some(path.ty),
                        Some(_) | None => {
                            errors.push(ValidationError::MissingPlaceProjection {
                                block,
                                location,
                                projection,
                            });
                            None
                        }
                    }
                } else {
                    Some(local.ty)
                }
            }
            None => {
                errors.push(ValidationError::MissingOperandLocal {
                    block,
                    location,
                    local: place.local,
                });
                None
            }
        },
    }
}

fn place_projection(body: &Body, place: super::Place) -> Option<&ProjectionPath> {
    let projection = body.projections.get(place.projection?.index())?;
    (projection.base == place.local).then_some(projection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::locals::{Local, LocalOrigin, LocalStorage};
    use crate::mir::model::{BasicBlock, CallArgument, Constant, Operand, Place, Rvalue};
    use crate::semantic::{BodyId, DefId, ExprId};
    use crate::source::{ByteSpan, SourceId};

    fn span() -> ByteSpan {
        ByteSpan::new(SourceId::new(0), 0, 1)
    }

    fn valid_body() -> Body {
        let ty = TyId::from_index(0);
        let root_scope = crate::mir::ScopeId::from_index(0);
        Body {
            source_body: BodyId::from_index(0),
            source_span: span(),
            return_local: LocalId::from_index(0),
            return_mode: crate::mir::ReturnMode::Plain,
            root_scope,
            scopes: vec![crate::mir::Scope::root(span())],
            locals: vec![Local::scalar(
                ty,
                ScalarType::I32,
                LocalStorage::Return,
                LocalOrigin::Return,
                root_scope,
            )],
            entry: BasicBlockId::from_index(0),
            blocks: vec![
                BasicBlock {
                    scope: root_scope,
                    statements: vec![Statement::Assign {
                        destination: Place::local(LocalId::from_index(0)),
                        value: Rvalue::Use(Operand::Constant(Constant {
                            ty,
                            scalar: ScalarType::I32,
                            value: 7,
                        })),
                        origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
                    }],
                    terminator: Terminator::Goto {
                        target: BasicBlockId::from_index(1),
                    },
                },
                BasicBlock {
                    scope: root_scope,
                    statements: Vec::new(),
                    terminator: Terminator::Return,
                },
            ],
            loop_regions: Vec::new(),
            loans: Vec::new(),
            projections: Vec::new(),
        }
    }

    #[test]
    fn accepts_a_well_formed_body() {
        assert_eq!(validate(&valid_body()), Ok(()));
    }

    #[test]
    fn rejects_unary_operator_and_scalar_drift() {
        let mut body = valid_body();
        let ty = body.locals[0].ty;
        body.blocks[0].statements[0] = Statement::Assign {
            destination: Place::local(LocalId::from_index(0)),
            value: Rvalue::Unary {
                operator: crate::mir::UnaryOperator::LogicalNot,
                operand: Operand::Constant(Constant {
                    ty,
                    scalar: ScalarType::I32,
                    value: 0,
                }),
                ty,
            },
            origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
        };

        assert_eq!(
            validate(&body),
            Err(vec![ValidationError::InvalidUnaryOperation {
                block: BasicBlockId::from_index(0),
                statement: 0,
                operator: crate::mir::UnaryOperator::LogicalNot,
                scalar: ScalarType::I32,
            }])
        );
    }

    #[test]
    fn rejects_failure_propagation_from_a_plain_body() {
        let mut body = valid_body();
        body.blocks[1].terminator = Terminator::PropagateFailure;

        assert_eq!(
            validate(&body),
            Err(vec![ValidationError::PropagationFromPlainBody {
                block: BasicBlockId::from_index(1),
            }])
        );

        body.return_mode = crate::mir::ReturnMode::Fallible;
        assert_eq!(validate(&body), Ok(()));
    }

    #[test]
    fn reports_identity_and_type_errors_together() {
        let mut body = valid_body();
        body.return_local = LocalId::from_index(4);
        body.blocks[0].statements[0] = Statement::Assign {
            destination: Place::local(LocalId::from_index(0)),
            value: Rvalue::Use(Operand::Constant(Constant {
                ty: TyId::from_index(1),
                scalar: ScalarType::I32,
                value: 7,
            })),
            origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
        };
        body.blocks[0].terminator = Terminator::Goto {
            target: BasicBlockId::from_index(8),
        };

        assert_eq!(
            validate(&body),
            Err(vec![
                ValidationError::MissingReturnLocal(LocalId::from_index(4)),
                ValidationError::AssignmentTypeMismatch {
                    block: BasicBlockId::from_index(0),
                    statement: 0,
                    destination: TyId::from_index(0),
                    value: TyId::from_index(1),
                },
                ValidationError::MissingTarget {
                    block: BasicBlockId::from_index(0),
                    target: BasicBlockId::from_index(8),
                },
            ])
        );
    }

    #[test]
    fn reports_call_edge_and_argument_invariants_together() {
        let mut body = valid_body();
        body.blocks[0].statements.clear();
        body.blocks[0].terminator = Terminator::Call {
            origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
            callee: DefId::from_index(0),
            arguments: vec![CallArgument {
                operand: Operand::Constant(Constant {
                    ty: TyId::from_index(1),
                    scalar: ScalarType::Usize,
                    value: 7,
                }),
                ty: TyId::from_index(0),
                representation: ValueRepresentation::Scalar(ScalarType::I32),
            }],
            continuation: CallContinuation::Return {
                destination: Place::local(LocalId::from_index(4)),
                target: BasicBlockId::from_index(8),
            },
        };

        assert_eq!(
            validate(&body),
            Err(vec![
                ValidationError::MissingTarget {
                    block: BasicBlockId::from_index(0),
                    target: BasicBlockId::from_index(8),
                },
                ValidationError::MissingCallDestination {
                    block: BasicBlockId::from_index(0),
                    local: LocalId::from_index(4),
                },
                ValidationError::OperandTypeMismatch {
                    block: BasicBlockId::from_index(0),
                    location: OperandLocation::CallArgument(0),
                    expected: TyId::from_index(0),
                    actual: TyId::from_index(1),
                },
                ValidationError::OperandRepresentationMismatch {
                    block: BasicBlockId::from_index(0),
                    location: OperandLocation::CallArgument(0),
                    expected: ValueRepresentation::Scalar(ScalarType::I32),
                    actual: ValueRepresentation::Scalar(ScalarType::Usize),
                },
            ])
        );
    }

    #[test]
    fn rejects_local_storage_origin_and_ownership_drift() {
        let mut body = valid_body();
        let ty = body.locals[0].ty;
        let mut invalid = Local::scalar(
            ty,
            ScalarType::I32,
            LocalStorage::Parameter { ordinal: 0 },
            LocalOrigin::Desugared(span()),
            body.root_scope,
        );
        invalid.ownership = OwnershipKind::Move;
        body.locals.push(invalid);

        assert_eq!(
            validate(&body),
            Err(vec![ValidationError::InvalidLocalContract(
                LocalId::from_index(1)
            )])
        );
    }

    #[test]
    fn rejects_duplicate_parameter_storage() {
        let mut body = valid_body();
        let ty = body.locals[0].ty;
        for _ in 0..2 {
            body.locals.push(Local::scalar(
                ty,
                ScalarType::I32,
                LocalStorage::Parameter { ordinal: 0 },
                LocalOrigin::Desugared(span()),
                body.root_scope,
            ));
        }

        assert_eq!(
            validate(&body),
            Err(vec![
                ValidationError::InvalidLocalContract(LocalId::from_index(1)),
                ValidationError::InvalidLocalContract(LocalId::from_index(2)),
                ValidationError::DuplicateParameterStorage {
                    first: LocalId::from_index(1),
                    duplicate: LocalId::from_index(2),
                    index: 0,
                },
            ])
        );
    }

    #[test]
    fn accepts_identity_backed_aggregate_field_projection() {
        let mut body = valid_body();
        let base = LocalId::from_index(body.locals.len());
        let ty = body.locals[0].ty;
        body.locals.push(Local::aggregate(
            ty,
            OwnershipKind::Copy,
            LocalStorage::Local,
            LocalOrigin::Desugared(span()),
            body.root_scope,
        ));
        body.projections.push(ProjectionPath {
            id: ProjectionPathId::from_index(0),
            base,
            parent: None,
            element: ProjectionElement::Field { offset: 8 },
            ty,
            representation: ValueRepresentation::Scalar(ScalarType::I32),
            ownership: OwnershipKind::Copy,
        });

        assert_eq!(validate(&body), Ok(()));
    }

    #[test]
    fn rejects_projection_from_scalar_base() {
        let mut body = valid_body();
        body.projections.push(ProjectionPath {
            id: ProjectionPathId::from_index(0),
            base: body.return_local,
            parent: None,
            element: ProjectionElement::Field { offset: 0 },
            ty: body.locals[0].ty,
            representation: ValueRepresentation::Scalar(ScalarType::I32),
            ownership: OwnershipKind::Copy,
        });

        assert!(matches!(
            validate(&body),
            Err(errors) if errors.contains(&ValidationError::ProjectionOfScalar {
                projection: ProjectionPathId::from_index(0),
            })
        ));
    }

    #[test]
    fn rejects_forward_projection_parent() {
        let mut body = valid_body();
        let base = LocalId::from_index(body.locals.len());
        let ty = body.locals[0].ty;
        body.locals.push(Local::aggregate(
            ty,
            OwnershipKind::Copy,
            LocalStorage::Local,
            LocalOrigin::Desugared(span()),
            body.root_scope,
        ));
        body.projections.push(ProjectionPath {
            id: ProjectionPathId::from_index(0),
            base,
            parent: Some(ProjectionPathId::from_index(1)),
            element: ProjectionElement::Field { offset: 0 },
            ty,
            representation: ValueRepresentation::Aggregate,
            ownership: OwnershipKind::Copy,
        });

        assert!(matches!(
            validate(&body),
            Err(errors) if errors.contains(&ValidationError::InvalidProjectionParent {
                projection: ProjectionPathId::from_index(0),
                parent: ProjectionPathId::from_index(1),
            })
        ));
    }

    #[test]
    fn rejects_missing_local_scope() {
        let mut body = valid_body();
        body.locals[0].scope = crate::mir::ScopeId::from_index(4);

        assert_eq!(
            validate(&body),
            Err(vec![ValidationError::MissingLocalScope {
                local: LocalId::from_index(0),
                scope: crate::mir::ScopeId::from_index(4),
            }])
        );
    }

    #[test]
    fn rejects_a_scope_whose_parent_is_not_earlier() {
        let mut body = valid_body();
        body.scopes.push(crate::mir::Scope::child(
            crate::mir::ScopeId::from_index(1),
            span(),
        ));

        assert_eq!(
            validate(&body),
            Err(vec![ValidationError::InvalidScopeParent {
                scope: crate::mir::ScopeId::from_index(1),
                parent: crate::mir::ScopeId::from_index(1),
            }])
        );
    }

    #[test]
    fn rejects_a_copy_from_an_uninitialized_local() {
        let mut body = valid_body();
        let ty = body.locals[0].ty;
        body.locals.push(Local::scalar(
            ty,
            ScalarType::I32,
            LocalStorage::Local,
            LocalOrigin::Desugared(span()),
            body.root_scope,
        ));
        body.blocks[0].statements[0] = Statement::Assign {
            destination: Place::local(body.return_local),
            value: Rvalue::Use(Operand::Copy(Place::local(LocalId::from_index(1)))),
            origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
        };

        assert_eq!(
            validate(&body),
            Err(vec![ValidationError::Initialization(
                super::super::initialization::InitializationError {
                    block: BasicBlockId::from_index(0),
                    location: super::super::initialization::InitializationLocation::Statement(0),
                    local: LocalId::from_index(1),
                }
            )])
        );
    }

    #[test]
    fn rejects_a_return_local_not_initialized_on_every_join_path() {
        let mut body = valid_body();
        let ty = body.locals[0].ty;
        let value = LocalId::from_index(1);
        body.locals.push(Local::scalar(
            ty,
            ScalarType::I32,
            LocalStorage::Local,
            LocalOrigin::Desugared(span()),
            body.root_scope,
        ));
        body.blocks = vec![
            BasicBlock {
                scope: body.root_scope,
                statements: Vec::new(),
                terminator: Terminator::Switch {
                    condition: Operand::Constant(Constant {
                        ty,
                        scalar: ScalarType::Bool,
                        value: 1,
                    }),
                    then_target: BasicBlockId::from_index(1),
                    else_target: BasicBlockId::from_index(2),
                },
            },
            BasicBlock {
                scope: body.root_scope,
                statements: vec![Statement::Assign {
                    destination: Place::local(value),
                    value: Rvalue::Use(Operand::Constant(Constant {
                        ty,
                        scalar: ScalarType::I32,
                        value: 7,
                    })),
                    origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
                }],
                terminator: Terminator::Goto {
                    target: BasicBlockId::from_index(3),
                },
            },
            BasicBlock {
                scope: body.root_scope,
                statements: Vec::new(),
                terminator: Terminator::Goto {
                    target: BasicBlockId::from_index(3),
                },
            },
            BasicBlock {
                scope: body.root_scope,
                statements: vec![Statement::Assign {
                    destination: Place::local(body.return_local),
                    value: Rvalue::Use(Operand::Copy(Place::local(value))),
                    origin: crate::mir::Origin::Expression(ExprId::from_index(0)),
                }],
                terminator: Terminator::Return,
            },
        ];

        assert_eq!(
            validate(&body),
            Err(vec![ValidationError::Initialization(
                super::super::initialization::InitializationError {
                    block: BasicBlockId::from_index(3),
                    location: super::super::initialization::InitializationLocation::Statement(0),
                    local: value,
                }
            )])
        );
    }
}
