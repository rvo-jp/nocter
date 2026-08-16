use nocter_declarations::{CallableKind, ParameterRole, StructuralCapability};
use nocter_model::{BorrowCapability, CallableCapability, CallableId, Symbol, TypeId, TypeKind};

use super::selection::{
    InstanceOperationSelector, InstanceSelectionError, selected_generic_arguments, visible_callable,
};
use crate::conformance::MethodSelection;
use crate::type_relations::{TypeSubstitution, is_concrete_type, match_type_pattern};
use crate::{
    CheckedPredicate, CoercedReceiverPreparation, GenericArgument, GenericArguments,
    StaticDispatch, StaticSelection,
};

pub(crate) struct MethodReceiverCoercion {
    source_capability: BorrowCapability,
    selection: StaticSelection,
    result_preparation: CoercedReceiverPreparation,
}

impl MethodReceiverCoercion {
    pub(crate) const fn source_capability(&self) -> BorrowCapability {
        self.source_capability
    }

    pub(crate) const fn selection(&self) -> &StaticSelection {
        &self.selection
    }

    pub(crate) const fn result_preparation(&self) -> CoercedReceiverPreparation {
        self.result_preparation
    }
}

/// One exact method contract and static implementation after owner specialization.
pub(crate) struct MethodCandidate {
    callable: CallableId,
    surface: CallableId,
    receiver_capability: CallableCapability,
    generic_arguments: GenericArguments,
    substitution: TypeSubstitution,
    dispatch: StaticDispatch,
    receiver_coercion: Option<MethodReceiverCoercion>,
}

impl MethodCandidate {
    pub(crate) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(crate) const fn surface(&self) -> CallableId {
        self.surface
    }

    pub(crate) const fn receiver_capability(&self) -> CallableCapability {
        self.receiver_capability
    }

    pub(crate) const fn generic_arguments(&self) -> &GenericArguments {
        &self.generic_arguments
    }

    pub(crate) const fn substitution(&self) -> &TypeSubstitution {
        &self.substitution
    }

    pub(crate) const fn dispatch(&self) -> StaticDispatch {
        self.dispatch
    }

    pub(crate) const fn receiver_coercion(&self) -> Option<&MethodReceiverCoercion> {
        self.receiver_coercion.as_ref()
    }
}

impl InstanceOperationSelector<'_> {
    /// Selects all exact-receiver inherent and interface method candidates with one name.
    ///
    /// Concrete receivers use explicit conformances. Unresolved generic receivers use only their
    /// lexical interface requirements. Callable-generic requirements remain for call planning
    /// because their substitution depends on arguments and result context.
    pub(crate) fn select_method_candidates(
        &mut self,
        target: TypeId,
        name: Symbol,
    ) -> Result<Vec<MethodCandidate>, InstanceSelectionError> {
        let mut selected = self.select_inherent_methods(target, name)?;
        if is_concrete_type(self.types, target)? {
            selected.extend(self.select_conformance_methods(target, name)?);
        } else {
            selected.extend(self.select_lexical_interface_methods(target, name)?);
        }
        Ok(selected)
    }

    /// Selects one-step borrow-coercion routes only after exact receiver lookup found nothing.
    ///
    /// A readwrite source first tries readonly coercion receivers. Readwrite coercion receivers
    /// are considered only when the minimum-authority tier yields no target method.
    pub(crate) fn select_coerced_method_candidates(
        &mut self,
        source: TypeId,
        name: Symbol,
        available: BorrowCapability,
    ) -> Result<Vec<MethodCandidate>, InstanceSelectionError> {
        let preferred = BorrowCapability::Readonly;
        let selected = self.method_candidates_through_coercions(source, name, preferred)?;
        if !selected.is_empty() || available == BorrowCapability::Readonly {
            return Ok(selected);
        }
        self.method_candidates_through_coercions(source, name, BorrowCapability::ReadWrite)
    }

    fn method_candidates_through_coercions(
        &mut self,
        source: TypeId,
        name: Symbol,
        source_capability: BorrowCapability,
    ) -> Result<Vec<MethodCandidate>, InstanceSelectionError> {
        let coercions = self.select_coercions(source, source_capability)?;
        let mut selected = Vec::new();
        for coercion in coercions {
            for mut method in self.select_method_candidates(coercion.target(), name)? {
                let result_preparation =
                    match (coercion.result_capability(), method.receiver_capability()) {
                        (BorrowCapability::Readonly, CallableCapability::Readonly) => {
                            CoercedReceiverPreparation::PreserveReadonly
                        }
                        (BorrowCapability::ReadWrite, CallableCapability::Readonly) => {
                            CoercedReceiverPreparation::WeakenReadwrite
                        }
                        (BorrowCapability::ReadWrite, CallableCapability::ReadWrite) => {
                            CoercedReceiverPreparation::PreserveReadwrite
                        }
                        (BorrowCapability::Readonly, CallableCapability::ReadWrite)
                        | (_, CallableCapability::Owned) => continue,
                    };
                method.receiver_coercion = Some(MethodReceiverCoercion {
                    source_capability: coercion.receiver_capability(),
                    selection: coercion.selection().clone(),
                    result_preparation,
                });
                selected.push(method);
            }
        }
        Ok(selected)
    }

    fn select_inherent_methods(
        &mut self,
        target: TypeId,
        name: Symbol,
    ) -> Result<Vec<MethodCandidate>, InstanceSelectionError> {
        let mut selected = Vec::new();
        for applicable in self.applicable_instances(target)? {
            let members = self
                .table
                .entries()
                .get(applicable.instance)
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
                if callable.kind() != CallableKind::Method
                    || callable.name() != Some(name)
                    || !visible_callable(self.graph, self.from, callable.site())?
                {
                    continue;
                }
                let receiver_capability = receiver_capability(
                    self.graph,
                    self.types,
                    member,
                    &applicable.substitution,
                    target,
                )?;
                selected.push(MethodCandidate {
                    callable: member,
                    surface: member,
                    receiver_capability,
                    generic_arguments: applicable.generic_arguments.clone(),
                    substitution: applicable.substitution.clone(),
                    dispatch: StaticDispatch::Direct(member),
                    receiver_coercion: None,
                });
            }
        }
        Ok(selected)
    }

    fn select_lexical_interface_methods(
        &mut self,
        target: TypeId,
        name: Symbol,
    ) -> Result<Vec<MethodCandidate>, InstanceSelectionError> {
        let requirements = self.assumptions.to_vec();
        let mut selected = Vec::new();
        for requirement in requirements {
            let CheckedPredicate::Capability {
                subject,
                capability: StructuralCapability::Interface(application),
            } = requirement.predicate()
            else {
                continue;
            };
            if *subject != target {
                continue;
            }
            let interface_id = application.interface();
            let interface = self
                .graph
                .declarations()
                .interfaces()
                .get(interface_id)
                .ok_or(InstanceSelectionError::MissingInterface(interface_id))?;
            let associated_types = interface.associated_types().to_vec();
            let mut substitution = self.interface_substitution(target, application)?;
            self.bind_lexical_associated_types(
                interface_id,
                target,
                &associated_types,
                &mut substitution,
            )?;
            let generic_arguments = interface_generic_arguments(interface, application)?;
            for method in interface.methods() {
                let callable = self
                    .graph
                    .declarations()
                    .callables()
                    .get(*method)
                    .ok_or(InstanceSelectionError::MissingCallable(*method))?;
                if callable.name() != Some(name)
                    || !visible_callable(self.graph, self.from, callable.site())?
                {
                    continue;
                }
                let capability =
                    receiver_capability(self.graph, self.types, *method, &substitution, target)?;
                selected.push(MethodCandidate {
                    callable: *method,
                    surface: *method,
                    receiver_capability: capability,
                    generic_arguments: generic_arguments.clone(),
                    substitution: substitution.clone(),
                    dispatch: StaticDispatch::InterfaceMethod {
                        requirement: requirement.declaration(),
                        method: *method,
                    },
                    receiver_coercion: None,
                });
            }
        }
        Ok(selected)
    }

    fn select_conformance_methods(
        &mut self,
        target: TypeId,
        name: Symbol,
    ) -> Result<Vec<MethodCandidate>, InstanceSelectionError> {
        let interfaces = self.conformances.method_interfaces(name).to_vec();
        let mut selected = Vec::new();
        for interface_id in interfaces {
            let interface = self
                .graph
                .declarations()
                .interfaces()
                .get(interface_id)
                .ok_or(InstanceSelectionError::MissingInterface(interface_id))?;
            let surface = interface
                .methods()
                .iter()
                .copied()
                .find(|method| {
                    self.graph
                        .declarations()
                        .callables()
                        .get(*method)
                        .is_some_and(|callable| callable.name() == Some(name))
                })
                .ok_or(InstanceSelectionError::InvalidInterfaceMethod(interface_id))?;
            let surface_declaration = self
                .graph
                .declarations()
                .callables()
                .get(surface)
                .ok_or(InstanceSelectionError::MissingCallable(surface))?;
            if !visible_callable(self.graph, self.from, surface_declaration.site())? {
                continue;
            }
            for conformance_id in self.conformances.candidates(interface_id).to_vec() {
                let conformance = self
                    .conformances
                    .entries()
                    .get(conformance_id)
                    .cloned()
                    .ok_or(InstanceSelectionError::MissingConformance(conformance_id))?;
                let mut pattern_substitution = TypeSubstitution::default();
                for refinement in conformance.refinements() {
                    pattern_substitution.bind_generic(refinement.parameter(), refinement.ty());
                }
                let Some(bindings) = match_type_pattern(self.types, conformance.target(), target)?
                else {
                    continue;
                };
                for (parameter, ty) in bindings.iter() {
                    pattern_substitution.bind_generic(parameter, ty);
                }
                if !self.requirements_hold(conformance.requirements(), &pattern_substitution)? {
                    continue;
                }
                let selection = conformance
                    .method(surface)
                    .ok_or(InstanceSelectionError::InvalidMethodSignature(surface))?;
                let (callable, substitution, generic_arguments) = match selection {
                    MethodSelection::Implementation(callable) => {
                        let arguments = selected_generic_arguments(
                            self.types,
                            conformance.generic_parameters(),
                            &pattern_substitution,
                        )?;
                        (callable, pattern_substitution, arguments)
                    }
                    MethodSelection::Default(callable) => {
                        let application = specialized_application(
                            self.types,
                            conformance.interface(),
                            &pattern_substitution,
                        )?;
                        let mut substitution = self.interface_substitution(target, &application)?;
                        for binding in conformance.associated_types() {
                            substitution.bind_associated(
                                binding.declaration(),
                                pattern_substitution.apply_type(self.types, binding.ty())?,
                            );
                        }
                        let arguments = interface_generic_arguments(interface, &application)?;
                        (callable, substitution, arguments)
                    }
                };
                let capability =
                    receiver_capability(self.graph, self.types, callable, &substitution, target)?;
                selected.push(MethodCandidate {
                    callable,
                    surface,
                    receiver_capability: capability,
                    generic_arguments,
                    substitution,
                    dispatch: StaticDispatch::Direct(callable),
                    receiver_coercion: None,
                });
            }
        }
        Ok(selected)
    }

    fn interface_substitution(
        &self,
        target: TypeId,
        application: &nocter_declarations::InterfaceApplication,
    ) -> Result<TypeSubstitution, InstanceSelectionError> {
        let interface = self
            .graph
            .declarations()
            .interfaces()
            .get(application.interface())
            .ok_or(InstanceSelectionError::MissingInterface(
                application.interface(),
            ))?;
        if interface.generic_parameters().len() != application.arguments().len() {
            return Err(InstanceSelectionError::InvalidInterfaceMethod(
                application.interface(),
            ));
        }
        let mut substitution = TypeSubstitution::default();
        substitution.set_interface_self(application.interface(), target);
        for (parameter, argument) in interface
            .generic_parameters()
            .iter()
            .copied()
            .zip(application.arguments().iter().copied())
        {
            substitution.bind_generic(parameter, argument);
        }
        Ok(substitution)
    }

    fn bind_lexical_associated_types(
        &mut self,
        interface: nocter_model::InterfaceId,
        target: TypeId,
        declarations: &[nocter_model::AssociatedTypeId],
        substitution: &mut TypeSubstitution,
    ) -> Result<(), InstanceSelectionError> {
        for associated in declarations {
            let projection = self
                .types
                .intern(TypeKind::AssociatedProjection {
                    base: target,
                    associated: *associated,
                })
                .map_err(|_| InstanceSelectionError::UnknownType(target))?;
            substitution.bind_associated(*associated, projection);
        }
        for requirement in self.assumptions {
            let CheckedPredicate::TypeEquality { left, right } = requirement.predicate() else {
                continue;
            };
            if let Some(associated) =
                associated_projection(self.graph, self.types, *left, interface, target)
            {
                substitution.bind_associated(associated, *right);
            } else if let Some(associated) =
                associated_projection(self.graph, self.types, *right, interface, target)
            {
                substitution.bind_associated(associated, *left);
            }
        }
        Ok(())
    }
}

fn receiver_capability(
    graph: &nocter_declarations::DeclarationGraph,
    types: &mut nocter_model::TypeStore,
    callable: CallableId,
    substitution: &TypeSubstitution,
    target: TypeId,
) -> Result<CallableCapability, InstanceSelectionError> {
    let declaration = graph
        .declarations()
        .callables()
        .get(callable)
        .ok_or(InstanceSelectionError::MissingCallable(callable))?;
    if declaration.kind() != CallableKind::Method {
        return Err(InstanceSelectionError::InvalidMethodSignature(callable));
    }
    let receiver = declaration
        .receiver()
        .and_then(|receiver| graph.declarations().parameters().get(receiver))
        .ok_or(InstanceSelectionError::InvalidMethodSignature(callable))?;
    let ParameterRole::Receiver(capability) = receiver.role() else {
        return Err(InstanceSelectionError::InvalidMethodSignature(callable));
    };
    if substitution.apply_type(types, receiver.ty())? != target {
        return Err(InstanceSelectionError::InvalidMethodSignature(callable));
    }
    Ok(capability)
}

fn interface_generic_arguments(
    interface: &nocter_declarations::InterfaceDeclaration,
    application: &nocter_declarations::InterfaceApplication,
) -> Result<GenericArguments, InstanceSelectionError> {
    GenericArguments::new(
        interface
            .generic_parameters()
            .iter()
            .copied()
            .zip(application.arguments().iter().copied())
            .map(|(parameter, ty)| GenericArgument::new(parameter, ty)),
    )
    .map_err(|duplicate| InstanceSelectionError::DuplicateGeneric(duplicate.parameter()))
}

fn specialized_application(
    types: &mut nocter_model::TypeStore,
    application: &nocter_declarations::InterfaceApplication,
    substitution: &TypeSubstitution,
) -> Result<nocter_declarations::InterfaceApplication, InstanceSelectionError> {
    Ok(nocter_declarations::InterfaceApplication::new(
        application.interface(),
        application
            .arguments()
            .iter()
            .map(|argument| substitution.apply_type(types, *argument))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn associated_projection(
    graph: &nocter_declarations::DeclarationGraph,
    types: &nocter_model::TypeStore,
    ty: TypeId,
    interface: nocter_model::InterfaceId,
    target: TypeId,
) -> Option<nocter_model::AssociatedTypeId> {
    let TypeKind::AssociatedProjection { base, associated } = types.get(ty)? else {
        return None;
    };
    let declaration = graph.declarations().associated_types().get(*associated)?;
    if declaration.interface() != interface {
        return None;
    }
    let belongs = matches!(
        types.get(*base),
        Some(TypeKind::InterfaceSelf(actual)) if *actual == interface
    ) || *base == target;
    belongs.then_some(*associated)
}
