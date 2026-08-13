//! Structural MIR verification. Validation errors are compiler invariant
//! failures, not alternate source-language diagnostics.

use super::ids::{BasicBlockId, LocalId};
use super::locals::{LocalOrigin, LocalStorage, OwnershipKind, ScalarType, ValueRepresentation};
use super::model::{Body, CallContinuation, Operand, Statement, Terminator};
use crate::semantic::TyId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ValidationError {
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
    AssignmentRequiresScalar {
        block: BasicBlockId,
        statement: usize,
        actual: ValueRepresentation,
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
}

pub(crate) fn validate(body: &Body) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    if body.locals.get(body.return_local.index()).is_none() {
        errors.push(ValidationError::MissingReturnLocal(body.return_local));
    }
    validate_scopes(body, &mut errors);
    validate_local_contracts(body, &mut errors);
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
            let Statement::Assign {
                destination, value, ..
            } = statement;
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
                    ty
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
                        ValueRepresentation::Aggregate => {
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
                    validate_operand(
                        body,
                        block_id,
                        OperandLocation::CallArgument(index),
                        &argument.operand,
                        argument.ty,
                        argument.scalar,
                        &mut errors,
                    );
                }
            }
            Terminator::PropagateFailure if body.return_mode == super::ReturnMode::Plain => {
                errors.push(ValidationError::PropagationFromPlainBody { block: block_id });
            }
            Terminator::Trap | Terminator::PropagateFailure | Terminator::Return => {}
        }
    }

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
                | (LocalStorage::Parameter(_), LocalOrigin::Parameter(_))
                | (
                    LocalStorage::Local,
                    LocalOrigin::Binding(_) | LocalOrigin::Temporary(_) | LocalOrigin::Desugared(_)
                )
        );
        let return_storage_matches_identity = !return_local_exists
            || (id == body.return_local) == (local.storage == LocalStorage::Return);
        let scalar_ownership_is_trivial =
            !matches!(local.representation, ValueRepresentation::Scalar(_))
                || local.ownership == OwnershipKind::Trivial;
        if !storage_matches_origin
            || !return_storage_matches_identity
            || !scalar_ownership_is_trivial
        {
            errors.push(ValidationError::InvalidLocalContract(id));
        }
        if let LocalStorage::Parameter(parameter_index) = local.storage
            && let Some(first) = parameter_storage.insert(parameter_index, id)
        {
            errors.push(ValidationError::DuplicateParameterStorage {
                first,
                duplicate: id,
                index: parameter_index,
            });
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
    actual_ty
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
        Some(actual @ ValueRepresentation::Aggregate) => {
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
        Operand::Copy(place) => body
            .locals
            .get(place.local.index())
            .map(|local| local.representation),
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
        Operand::Copy(place) => match body.locals.get(place.local.index()) {
            Some(local) => Some(local.ty),
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
                        destination: Place {
                            local: LocalId::from_index(0),
                        },
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
        }
    }

    #[test]
    fn accepts_a_well_formed_body() {
        assert_eq!(validate(&valid_body()), Ok(()));
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
            destination: Place {
                local: LocalId::from_index(0),
            },
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
                scalar: ScalarType::I32,
            }],
            continuation: CallContinuation::Return {
                destination: Place {
                    local: LocalId::from_index(4),
                },
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
                ValidationError::OperandScalarMismatch {
                    block: BasicBlockId::from_index(0),
                    location: OperandLocation::CallArgument(0),
                    expected: ScalarType::I32,
                    actual: ScalarType::Usize,
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
            LocalStorage::Parameter(0),
            LocalOrigin::Desugared(span()),
            body.root_scope,
        );
        invalid.ownership = OwnershipKind::Owned;
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
                LocalStorage::Parameter(0),
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
}
