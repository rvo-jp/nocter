use nocter_declarations::{CallableKind, NominalShape, ParameterRole};
use nocter_model::{BorrowCapability, BuiltinType, CallableCapability, TypeId, TypeKind};

use super::selection::{InstanceOperationSelector, InstanceSelectionError, borrow_result};
use crate::conformance::normalize_requirements;
use crate::type_relations::TypeSubstitution;
use crate::{
    CheckedPredicate, ComparisonOperation, GenericArguments, StaticDispatch, StaticSelection,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ComparisonCandidateImplementation {
    Primitive,
    Selected(StaticSelection),
}

/// One complete semantic comparison candidate before source-order operands are attached.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ComparisonOperationCandidate {
    implementation: ComparisonCandidateImplementation,
    receiver_coercion: Option<StaticSelection>,
    argument_coercion: Option<StaticSelection>,
}

impl ComparisonOperationCandidate {
    pub(crate) const fn implementation(&self) -> &ComparisonCandidateImplementation {
        &self.implementation
    }

    pub(crate) const fn receiver_coercion(&self) -> Option<&StaticSelection> {
        self.receiver_coercion.as_ref()
    }

    pub(crate) const fn argument_coercion(&self) -> Option<&StaticSelection> {
        self.argument_coercion.as_ref()
    }
}

impl InstanceOperationSelector<'_> {
    /// Selects primitive, structural, or source-defined comparison implementations.
    ///
    /// `left` is the semantic receiver, which may differ from the source-left operand for a
    /// derived strict-order token. Primitive implementations have absolute priority. A viable
    /// exact receiver declaration has priority over every receiver-coercion route.
    pub(crate) fn select_comparison_operations(
        &mut self,
        left: TypeId,
        right: TypeId,
        operation: ComparisonOperation,
    ) -> Result<Vec<ComparisonOperationCandidate>, InstanceSelectionError> {
        if left == right && self.primitive_comparison(left, operation)? {
            return Ok(vec![ComparisonOperationCandidate {
                implementation: ComparisonCandidateImplementation::Primitive,
                receiver_coercion: None,
                argument_coercion: None,
            }]);
        }

        let mut direct = Vec::new();
        for selection in self.select_direct_comparison_operations(left, operation)? {
            for argument_coercion in self.comparison_argument_routes(right, left)? {
                direct.push(ComparisonOperationCandidate {
                    implementation: ComparisonCandidateImplementation::Selected(selection.clone()),
                    receiver_coercion: None,
                    argument_coercion,
                });
            }
        }
        if !direct.is_empty() {
            return Ok(direct);
        }

        let mut coerced = Vec::new();
        for receiver_coercion in self.select_borrow_coercions(
            left,
            BorrowCapability::Readonly,
            BorrowCapability::Readonly,
        )? {
            for selection in
                self.select_direct_comparison_operations(receiver_coercion.target, operation)?
            {
                for argument_coercion in
                    self.comparison_argument_routes(right, receiver_coercion.target)?
                {
                    coerced.push(ComparisonOperationCandidate {
                        implementation: ComparisonCandidateImplementation::Selected(
                            selection.clone(),
                        ),
                        receiver_coercion: Some(receiver_coercion.selection.clone()),
                        argument_coercion,
                    });
                }
            }
        }
        Ok(coerced)
    }

    fn select_direct_comparison_operations(
        &mut self,
        target: TypeId,
        operation: ComparisonOperation,
    ) -> Result<Vec<StaticSelection>, InstanceSelectionError> {
        let predicate = match operation {
            ComparisonOperation::Equal => CheckedPredicate::Equality(target),
            ComparisonOperation::Less => CheckedPredicate::Ordering(target),
        };
        let mut selected = self
            .assumptions
            .iter()
            .filter(|assumption| assumption.predicate() == &predicate)
            .map(|assumption| {
                StaticSelection::new(
                    StaticDispatch::StructuralRequirement(assumption.declaration()),
                    GenericArguments::default(),
                )
            })
            .collect::<Vec<_>>();

        let callable_kind = match operation {
            ComparisonOperation::Equal => CallableKind::Equality,
            ComparisonOperation::Less => CallableKind::Ordering,
        };
        for applicable in self.applicable_instances(target)? {
            let members = self
                .table
                .entries()
                .get(&applicable.instance)
                .ok_or(InstanceSelectionError::MissingInstance(applicable.instance))?
                .members()
                .to_vec();
            for member in members {
                let callable = self
                    .graph
                    .declarations()
                    .callables()
                    .get(member)
                    .ok_or(InstanceSelectionError::MissingCallable(member))?;
                if callable.kind() != callable_kind
                    || !self.callable_is_admissible(callable.site())?
                {
                    continue;
                }
                let receiver = callable
                    .receiver()
                    .and_then(|receiver| self.graph.declarations().parameters().get(receiver))
                    .ok_or(InstanceSelectionError::InvalidComparisonSignature(member))?;
                let receiver_ty = applicable
                    .substitution
                    .apply_type(self.types, receiver.ty())?;
                if receiver.role() != ParameterRole::Receiver(CallableCapability::Readonly)
                    || receiver_ty != target
                    || callable.parameters().len() != 1
                    || !callable.generic_parameters().is_empty()
                {
                    return Err(InstanceSelectionError::InvalidComparisonSignature(member));
                }
                let parameter_id = callable.parameters()[0];
                let parameter = self
                    .graph
                    .declarations()
                    .parameters()
                    .get(parameter_id)
                    .ok_or(InstanceSelectionError::MissingParameter(parameter_id))?;
                if parameter.role() != (ParameterRole::Ordinary { position: 0 }) {
                    return Err(InstanceSelectionError::InvalidComparisonSignature(member));
                }
                let parameter_ty = applicable
                    .substitution
                    .apply_type(self.types, parameter.ty())?;
                let result = applicable
                    .substitution
                    .apply_type(self.types, callable.result())?;
                if borrow_result(self.types, parameter_ty)
                    != Some((BorrowCapability::Readonly, target))
                    || result != self.types.builtin(BuiltinType::Bool)
                {
                    return Err(InstanceSelectionError::InvalidComparisonSignature(member));
                }
                let callable_requirements = normalize_requirements(
                    self.graph,
                    self.types,
                    &applicable.substitution,
                    callable.requirements(),
                )?;
                if !self.requirements_hold(&callable_requirements, &TypeSubstitution::default())? {
                    continue;
                }
                selected.push(StaticSelection::new(
                    StaticDispatch::Direct(member),
                    applicable.generic_arguments.clone(),
                ));
            }
        }
        Ok(selected)
    }

    fn comparison_argument_routes(
        &mut self,
        source: TypeId,
        target: TypeId,
    ) -> Result<Vec<Option<StaticSelection>>, InstanceSelectionError> {
        if source == target {
            return Ok(vec![None]);
        }
        Ok(self
            .select_borrow_coercions(
                source,
                BorrowCapability::Readonly,
                BorrowCapability::Readonly,
            )?
            .into_iter()
            .filter(|coercion| coercion.target == target)
            .map(|coercion| Some(coercion.selection))
            .collect())
    }

    fn primitive_comparison(
        &self,
        ty: TypeId,
        operation: ComparisonOperation,
    ) -> Result<bool, InstanceSelectionError> {
        if integer_type(self.types, ty) {
            return Ok(true);
        }
        if operation == ComparisonOperation::Less {
            return Ok(false);
        }
        match self.types.get(ty) {
            Some(TypeKind::Builtin(BuiltinType::Bool)) => Ok(true),
            Some(TypeKind::Nominal { definition, .. }) => {
                let declaration = self
                    .graph
                    .declarations()
                    .nominal_types()
                    .get(*definition)
                    .ok_or(InstanceSelectionError::MissingNominal(*definition))?;
                let NominalShape::Enum { variants } = declaration.shape() else {
                    return Ok(false);
                };
                for variant in variants {
                    let declaration = self
                        .graph
                        .declarations()
                        .variants()
                        .get(*variant)
                        .ok_or(InstanceSelectionError::MissingVariant(*variant))?;
                    if !declaration.payload().is_empty() {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Some(_) => Ok(false),
            None => Err(InstanceSelectionError::UnknownType(ty)),
        }
    }
}

fn integer_type(types: &nocter_model::TypeStore, ty: TypeId) -> bool {
    matches!(
        types.get(ty),
        Some(TypeKind::Builtin(
            BuiltinType::U8
                | BuiltinType::U16
                | BuiltinType::U32
                | BuiltinType::U64
                | BuiltinType::Usize
                | BuiltinType::I8
                | BuiltinType::I16
                | BuiltinType::I32
                | BuiltinType::I64
                | BuiltinType::Isize
        ))
    )
}
