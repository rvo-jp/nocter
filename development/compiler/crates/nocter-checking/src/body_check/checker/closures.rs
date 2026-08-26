use std::mem;

use nocter_model::{
    BodyNodeId, BorrowCapability, CallableCapability, CallableContract, TypeId, TypeKind,
};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{NodeId, NodeKind};

use super::{BlockExpectation, BodyChecker};
use crate::body_check::closure_capability;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::copyability::Copyability;
use crate::syntax::{direct_child, direct_children, direct_identifier};
use crate::type_relations::{TypeSubstitution, collect_generic_parameters};
use crate::{
    CallableInference, CaptureMode, CheckedClosure, CheckedClosureCapture, CheckedOperation,
    ClosureDefinition, ClosureEnvironmentField, ClosureParameter, ClosureSignature,
    InferenceFailure, PlaceAccess,
};

struct CheckedClosureHead {
    parameters: Vec<ClosureParameter>,
    environment: Vec<ClosureEnvironmentField>,
    capture_initializers: Vec<CheckedClosureCapture>,
}

struct ClosureExpectation {
    capability: CallableCapability,
    parameters: Box<[TypeId]>,
    result: Option<TypeId>,
}

impl BodyChecker<'_, '_> {
    pub(super) fn check_closure(
        &mut self,
        node: NodeId,
        expected: Option<TypeId>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let expected_contract =
            self.expected_closure_contract(expected)?
                .map(|contract| ClosureExpectation {
                    capability: contract.capability(),
                    parameters: contract.parameters().into(),
                    result: Some(contract.result()),
                });
        self.check_closure_with_expectation(node, expected_contract.as_ref())
    }

    pub(super) fn constrain_closure_annotations(
        &mut self,
        node: NodeId,
        contract: &CallableContract,
        inference: &mut CallableInference,
    ) -> Result<(), BodyCheckError> {
        let head = direct_child(self.tree(), node, NodeKind::ClosureHead)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let parameters = direct_child(self.tree(), head, NodeKind::ClosureParameters)
            .map(|parameters| direct_children(self.tree(), parameters, NodeKind::ClosureParameter))
            .unwrap_or_default();
        if parameters.len() != contract.parameters().len() {
            return Err(self.rule(BodyRule::TypeMismatch, head)?);
        }
        for (parameter, expected) in parameters
            .into_iter()
            .zip(contract.parameters().iter().copied())
        {
            let Some(annotation) = direct_child(self.tree(), parameter, NodeKind::TypeAnnotation)
                .and_then(|annotation| direct_child(self.tree(), annotation, NodeKind::Type))
            else {
                continue;
            };
            inference.constrain_exact(expected, self.resolve_data_type_use(annotation)?);
        }
        Ok(())
    }

    pub(super) fn closure_context_is_ready(
        &mut self,
        contract: &CallableContract,
        inference_parameters: &[nocter_model::GenericParameterId],
        substitution: &TypeSubstitution,
    ) -> Result<bool, BodyCheckInternalError> {
        for parameter in contract.parameters().iter().copied() {
            let parameter = self.apply_type_substitution(substitution, parameter)?;
            let known = collect_generic_parameters(self.types, [parameter])
                .map(|generics| {
                    !generics
                        .iter()
                        .any(|generic| inference_parameters.contains(generic))
                })
                .map_err(InferenceFailure::from)
                .map_err(BodyCheckInternalError::CallInference)?;
            if !known {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn check_inferred_closure(
        &mut self,
        node: NodeId,
        contract: Option<&CallableContract>,
        inference_parameters: &[nocter_model::GenericParameterId],
        inference: &mut CallableInference,
        failure_rule: BodyRule,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let expectation = if let Some(contract) = contract {
            let substitution = inference
                .partial_substitution(self.types)
                .map_err(|error| self.inference_error(node, error, failure_rule))?;
            if !self.closure_context_is_ready(contract, inference_parameters, &substitution)? {
                let unresolved = contract
                    .parameters()
                    .iter()
                    .copied()
                    .flat_map(|parameter| {
                        collect_generic_parameters(self.types, [parameter])
                            .unwrap_or_default()
                            .into_iter()
                    })
                    .find(|parameter| inference_parameters.contains(parameter))
                    .ok_or(BodyCheckInternalError::CallContractSelection)?;
                return Err(self.inference_error(
                    node,
                    InferenceFailure::UnknownParameter(unresolved),
                    failure_rule,
                ));
            }
            let parameters = contract
                .parameters()
                .iter()
                .copied()
                .map(|parameter| self.apply_type_substitution(&substitution, parameter))
                .collect::<Result<Vec<_>, _>>()?;
            let result = self.apply_type_substitution(&substitution, contract.result())?;
            let result_generics = collect_generic_parameters(self.types, [result])
                .map_err(InferenceFailure::from)
                .map_err(BodyCheckInternalError::CallInference)?;
            Some(ClosureExpectation {
                capability: contract.capability(),
                parameters: parameters.into_boxed_slice(),
                result: (!result_generics
                    .iter()
                    .any(|parameter| inference_parameters.contains(parameter)))
                .then_some(result),
            })
        } else {
            None
        };
        let value = match self.check_closure_with_expectation(node, expectation.as_ref()) {
            Err(BodyCheckError::Rule {
                rule: BodyRule::TypeMismatch,
                ..
            }) => return Err(self.rule(failure_rule, node)?),
            result => result?,
        };
        if let Some(contract) = contract {
            let closure = match self.types.get(self.node_type(value)?) {
                Some(TypeKind::Closure { definition, .. }) => *definition,
                _ => return Err(BodyCheckInternalError::CallContractSelection.into()),
            };
            let signature = self
                .closures
                .get(closure)
                .ok_or(BodyCheckInternalError::MissingClosure(closure))?
                .signature()
                .clone();
            for (expected, actual) in contract
                .parameters()
                .iter()
                .copied()
                .zip(signature.parameter_types())
            {
                inference.constrain_exact(expected, actual);
            }
            inference.constrain_exact(contract.result(), signature.result());
        }
        Ok(value)
    }

    fn check_closure_with_expectation(
        &mut self,
        node: NodeId,
        expected: Option<&ClosureExpectation>,
    ) -> Result<BodyNodeId, BodyCheckError> {
        let closure = self
            .closure_ids
            .get(&node)
            .copied()
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let closure_type = self
            .types
            .intern(TypeKind::Closure {
                definition: closure,
                arguments: self.closure_type_arguments.clone(),
            })
            .map_err(|_| {
                BodyCheckInternalError::UnknownType(
                    self.types.builtin(nocter_model::BuiltinType::Void),
                )
            })?;
        let head = direct_child(self.tree(), node, NodeKind::ClosureHead)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let checked_head =
            self.check_closure_head(head, expected.map(|expected| expected.parameters.as_ref()))?;
        let declared_result = self.closure_result_type(node)?;
        let fixed_result = match (
            declared_result,
            expected.and_then(|expected| expected.result),
        ) {
            (Some(declared), Some(expected)) if declared != expected => {
                return Err(self.rule(BodyRule::TypeMismatch, node)?);
            }
            (Some(declared), _) => Some(declared),
            (None, Some(expected)) => Some(expected),
            (None, None) => None,
        };
        let block = direct_child(self.tree(), node, NodeKind::Block)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let (body, result) = self.check_nested_callable_block(block, fixed_result)?;
        let capability = closure_capability::infer(&self.builder, body)?;
        if let Some(expected) = expected
            && !closure_contract_accepts(
                expected,
                capability,
                &checked_head
                    .parameters
                    .iter()
                    .copied()
                    .map(ClosureParameter::ty)
                    .collect::<Vec<_>>(),
                result,
            )
        {
            return Err(self.rule(BodyRule::TypeMismatch, node)?);
        }
        self.copyabilities
            .register_closure(
                self.graph,
                self.types,
                closure_type,
                checked_head
                    .environment
                    .iter()
                    .copied()
                    .map(ClosureEnvironmentField::ty),
            )
            .map_err(BodyCheckInternalError::Copyability)?;
        let signature = ClosureSignature::new(capability, checked_head.parameters, result);
        self.closures.define(
            closure,
            ClosureDefinition::new(
                self.source.body(),
                closure_type,
                signature,
                checked_head.environment,
                body,
            ),
        )?;
        let value = self.add_node(
            node,
            closure_type,
            CheckedOperation::Closure(CheckedClosure::new(
                closure,
                checked_head.capture_initializers,
            )),
        )?;
        Ok(value)
    }

    fn expected_closure_contract(
        &self,
        expected: Option<TypeId>,
    ) -> Result<Option<CallableContract>, BodyCheckInternalError> {
        match expected.map(|expected| self.types.get(expected)) {
            Some(Some(TypeKind::Callable(contract))) if contract.pack().is_none() => {
                Ok(Some(contract.clone()))
            }
            Some(None) => Err(BodyCheckInternalError::UnknownType(expected.unwrap())),
            Some(Some(_)) | None => Ok(None),
        }
    }

    fn check_closure_head(
        &mut self,
        head: NodeId,
        expected: Option<&[TypeId]>,
    ) -> Result<CheckedClosureHead, BodyCheckError> {
        let capture_nodes = direct_child(self.tree(), head, NodeKind::ClosureCaptures)
            .map(|captures| direct_children(self.tree(), captures, NodeKind::ClosureCapture))
            .unwrap_or_default();
        let mut environment = Vec::with_capacity(capture_nodes.len());
        let mut capture_initializers = Vec::with_capacity(capture_nodes.len());
        for capture_node in capture_nodes {
            let token = direct_identifier(self.tree(), capture_node)
                .ok_or(BodyCheckInternalError::InvalidSyntax(capture_node))?;
            let capture = self
                .capture_declarations
                .get(&SyntaxOrigin::Token(token))
                .copied()
                .ok_or(BodyCheckInternalError::MissingCaptureDeclaration(
                    capture_node,
                ))?;
            let declaration = self.names.captures().get(capture).copied().ok_or(
                BodyCheckInternalError::MissingCaptureDeclaration(capture_node),
            )?;
            let place = self.target_place(capture_node, declaration.source())?;
            let (initializer, stored_type) =
                self.check_capture_initializer(capture_node, declaration.mode(), &place)?;
            self.builder.define_capture(capture, place.ty)?;
            environment.push(ClosureEnvironmentField::new(capture, stored_type));
            capture_initializers.push(CheckedClosureCapture::new(capture, initializer));
        }

        let parameter_nodes = direct_child(self.tree(), head, NodeKind::ClosureParameters)
            .map(|parameters| direct_children(self.tree(), parameters, NodeKind::ClosureParameter))
            .unwrap_or_default();
        if expected.is_some_and(|expected| expected.len() != parameter_nodes.len()) {
            return Err(self.rule(BodyRule::TypeMismatch, head)?);
        }
        let mut parameters = Vec::with_capacity(parameter_nodes.len());
        for (position, parameter_node) in parameter_nodes.into_iter().enumerate() {
            let token = direct_identifier(self.tree(), parameter_node)
                .ok_or(BodyCheckInternalError::InvalidSyntax(parameter_node))?;
            let parameter = self
                .local_declarations
                .get(&SyntaxOrigin::Token(token))
                .copied()
                .ok_or(BodyCheckInternalError::MissingLocalDeclaration(
                    parameter_node,
                ))?;
            let annotation = direct_child(self.tree(), parameter_node, NodeKind::TypeAnnotation)
                .and_then(|annotation| direct_child(self.tree(), annotation, NodeKind::Type));
            let expected_type = expected.map(|expected| expected[position]);
            let ty = match (annotation, expected_type) {
                (Some(annotation), Some(expected)) => {
                    let declared = self.resolve_data_type_use(annotation)?;
                    if declared != expected {
                        return Err(self.rule(BodyRule::TypeMismatch, parameter_node)?);
                    }
                    declared
                }
                (Some(annotation), None) => self.resolve_data_type_use(annotation)?,
                (None, Some(expected)) => expected,
                (None, None) => return Err(self.rule(BodyRule::TypeMismatch, parameter_node)?),
            };
            self.builder.define_local(parameter, ty)?;
            parameters.push(ClosureParameter::new(parameter, ty));
        }
        Ok(CheckedClosureHead {
            parameters,
            environment,
            capture_initializers,
        })
    }

    fn check_capture_initializer(
        &mut self,
        syntax: NodeId,
        mode: CaptureMode,
        place: &super::ResolvedPlace,
    ) -> Result<(BodyNodeId, TypeId), BodyCheckError> {
        let (operation, stored_type) = match mode {
            CaptureMode::Readonly => (
                CheckedOperation::Borrow {
                    capability: BorrowCapability::Readonly,
                    place: place.id,
                },
                self.types
                    .intern(TypeKind::Borrow {
                        capability: BorrowCapability::Readonly,
                        referent: place.ty,
                    })
                    .map_err(|_| BodyCheckInternalError::UnknownType(place.ty))?,
            ),
            CaptureMode::ReadWrite => {
                if !self.is_writable_place(place.id)? {
                    return Err(self.rule(BodyRule::InvalidReadWriteBorrow, syntax)?);
                }
                (
                    CheckedOperation::Borrow {
                        capability: BorrowCapability::ReadWrite,
                        place: place.id,
                    },
                    self.types
                        .intern(TypeKind::Borrow {
                            capability: BorrowCapability::ReadWrite,
                            referent: place.ty,
                        })
                        .map_err(|_| BodyCheckInternalError::UnknownType(place.ty))?,
                )
            }
            CaptureMode::Move => {
                if place.access != PlaceAccess::Owned
                    || matches!(self.types.get(place.ty), Some(TypeKind::Borrow { .. }))
                {
                    return Err(self.rule(BodyRule::InvalidMoveSource, syntax)?);
                }
                match self.classify_copyability(place.ty)? {
                    Copyability::Copy => return Err(self.rule(BodyRule::MoveCopyValue, syntax)?),
                    Copyability::MoveOnly => {}
                }
                (CheckedOperation::Move(place.id), place.ty)
            }
        };
        let initializer = self.add_node(syntax, stored_type, operation)?;
        Ok((initializer, stored_type))
    }

    fn closure_result_type(&mut self, node: NodeId) -> Result<Option<TypeId>, BodyCheckError> {
        let Some(result) = direct_child(self.tree(), node, NodeKind::ClosureResult) else {
            return Ok(None);
        };
        let ty = direct_child(self.tree(), result, NodeKind::Type)
            .ok_or(BodyCheckInternalError::InvalidSyntax(result))?;
        self.resolve_callable_result_type_use(ty).map(Some)
    }

    fn check_nested_callable_block(
        &mut self,
        block: NodeId,
        fixed_result: Option<TypeId>,
    ) -> Result<(BodyNodeId, TypeId), BodyCheckError> {
        let saved_result = self.result_type;
        let saved_loops = mem::take(&mut self.loops);
        let saved_reachable = self.flow_reachable;
        let saved_inference = self.closure_result_inference.take();
        let saved_opaque_result = self.opaque_result.take();
        self.result_type =
            fixed_result.unwrap_or_else(|| self.types.builtin(nocter_model::BuiltinType::Void));
        self.flow_reachable = true;
        self.closure_result_inference = fixed_result
            .is_none()
            .then(crate::body_check::checker::closure_results::ClosureResultInference::default);
        let checked = match fixed_result {
            Some(result) => self
                .check_block(block, BlockExpectation::Callable)
                .map(|body| (body, result)),
            None => self
                .check_block(block, BlockExpectation::Value(None))
                .and_then(|body| self.finish_inferred_closure_result(block, body)),
        };
        let closure_loops = mem::replace(&mut self.loops, saved_loops);
        self.closure_result_inference = saved_inference;
        self.opaque_result = saved_opaque_result;
        self.result_type = saved_result;
        self.flow_reachable = saved_reachable;
        if !closure_loops.is_empty() {
            return Err(BodyCheckInternalError::LoopStack.into());
        }
        checked
    }
}

fn closure_contract_accepts(
    expected: &ClosureExpectation,
    actual_capability: CallableCapability,
    actual_parameters: &[TypeId],
    actual_result: TypeId,
) -> bool {
    expected.capability.permits(actual_capability)
        && expected.parameters.as_ref() == actual_parameters
        && expected
            .result
            .is_none_or(|expected| expected == actual_result)
}

pub(super) fn concrete_closure_satisfies(
    expected: &CallableContract,
    actual: &ClosureSignature,
) -> bool {
    expected.capability().permits(actual.capability())
        && expected.pack().is_none()
        && actual
            .parameter_types()
            .eq(expected.parameters().iter().copied())
        && actual.result() == expected.result()
}
