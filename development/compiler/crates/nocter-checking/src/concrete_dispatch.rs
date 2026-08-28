use std::collections::BTreeMap;
use std::fmt;

use nocter_declarations::{ExpansionCapability, ParameterRole};
use nocter_model::{
    BorrowCapability, CallableCapability, CallableContract, CallableId, GenericParameterId,
    InterfaceId, OpaqueTypeId, RequirementId, TypeId, TypeKind, TypeStore,
};

use crate::instance_operations::{ComparisonCandidateImplementation, ConcreteEvidenceAuthority};
use crate::interface_implementation::substitute_predicate;
use crate::{
    CheckedPredicate, CheckedProgram, ComparisonOperation, GenericArgument, GenericArguments,
    InstanceSelectionError, StaticDispatch, StaticSelection, SubstitutionError, TypeSubstitution,
    is_concrete_type,
};

/// One source callable and the complete concrete generic domain required to generate its body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCallableDispatch {
    callable: CallableId,
    generic_arguments: GenericArguments,
    interface_self: Option<(InterfaceId, TypeId)>,
}

impl ResolvedCallableDispatch {
    #[must_use]
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn generic_arguments(&self) -> &GenericArguments {
        &self.generic_arguments
    }

    #[must_use]
    pub const fn interface_self(&self) -> Option<(InterfaceId, TypeId)> {
        self.interface_self
    }
}

/// A compiler-owned operation that needs no source callable body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedPrimitiveDispatch {
    Equality {
        subject: TypeId,
        operand: TypeId,
    },
    Ordering {
        subject: TypeId,
        operand: TypeId,
    },
    Index {
        capability: BorrowCapability,
        container: TypeId,
        receiver: TypeId,
        index: TypeId,
        result: TypeId,
    },
    BorrowWeakening {
        source: TypeId,
        target: TypeId,
    },
}

/// One ordered executable step produced by concrete static-dispatch resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedDispatchStep {
    Direct(ResolvedCallableDispatch),
    Primitive(ResolvedPrimitiveDispatch),
    /// Invocation through a concrete value selected by one structural callable contract.
    CallableValue {
        subject: TypeId,
        contract: CallableContract,
    },
}

/// Concrete representation opening required before invoking a method advertised by an opaque
/// result contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedOpaqueReceiver {
    definition: OpaqueTypeId,
    opaque: TypeId,
    witness: TypeId,
    source: TypeId,
    target: TypeId,
}

impl ResolvedOpaqueReceiver {
    #[must_use]
    pub const fn definition(self) -> OpaqueTypeId {
        self.definition
    }

    #[must_use]
    pub const fn opaque(self) -> TypeId {
        self.opaque
    }

    #[must_use]
    pub const fn witness(self) -> TypeId {
        self.witness
    }

    #[must_use]
    pub const fn source(self) -> TypeId {
        self.source
    }

    #[must_use]
    pub const fn target(self) -> TypeId {
        self.target
    }
}

/// The complete lowering plan for one checked static selection.
///
/// Composite operations retain their operand lanes. A flat step sequence cannot distinguish a
/// receiver coercion from an argument coercion and therefore is not a sufficient executable
/// contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedDispatchPlan {
    Invocation(ResolvedDispatchStep),
    OpaqueInvocation {
        receiver: ResolvedOpaqueReceiver,
        operation: ResolvedDispatchStep,
    },
    Comparison {
        left_coercion: Option<ResolvedDispatchStep>,
        right_coercion: Option<ResolvedDispatchStep>,
        operation: ResolvedDispatchStep,
    },
    Index {
        receiver_coercion: Option<ResolvedDispatchStep>,
        operation: ResolvedDispatchStep,
    },
}

/// Stateful specialization authority for checked dispatch edges.
///
/// The resolver owns a fork of the checked type store because applying concrete substitutions may
/// intern types that do not exist in generic HIR. The fork preserves every checked [`TypeId`] and
/// becomes the sole type authority for all plans produced by this resolver.
pub struct ConcreteDispatchResolver<'program> {
    pub(super) program: &'program CheckedProgram,
    semantics: crate::semantic_authority::SemanticTransaction,
    pub(super) destructions: BTreeMap<TypeId, Option<crate::ConcreteDestructionPlan>>,
}

struct SpecializedOpaqueWitness {
    definition: nocter_model::OpaqueTypeId,
    witness: TypeId,
    application: nocter_declarations::InterfaceApplication,
}

impl<'program> ConcreteDispatchResolver<'program> {
    #[must_use]
    pub fn new(program: &'program CheckedProgram) -> Self {
        Self {
            program,
            semantics: program.semantic_authority().transaction(),
            destructions: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn types(&self) -> &TypeStore {
        self.semantics.types()
    }

    pub(super) fn types_mut(&mut self) -> &mut nocter_model::TypeTransaction {
        self.semantics.types_mut()
    }

    pub(super) fn semantic_access(&mut self) -> crate::semantic_authority::SemanticAccess<'_> {
        self.semantics.access()
    }

    #[must_use]
    pub fn into_types(self) -> TypeStore {
        self.semantics.finish_specialized_types()
    }

    /// Resolves one checked dispatch edge under its enclosing callable specialization.
    ///
    /// # Errors
    ///
    /// Returns a typed failure for invalid semantic references, incomplete specialization,
    /// inapplicable or ambiguous concrete evidence, or an operation that cannot produce runtime
    /// dispatch.
    pub fn resolve(
        &mut self,
        selection: &StaticSelection,
        enclosing: &TypeSubstitution,
    ) -> Result<ResolvedDispatchPlan, ConcreteDispatchError> {
        let arguments = self.specialize_arguments(selection.generic_arguments(), enclosing)?;
        match selection.dispatch() {
            StaticDispatch::Direct(callable) => Ok(ResolvedDispatchPlan::Invocation(
                ResolvedDispatchStep::Direct(ResolvedCallableDispatch {
                    callable,
                    generic_arguments: arguments,
                    interface_self: None,
                }),
            )),
            StaticDispatch::InterfaceMethod { evidence, method } => {
                self.resolve_interface_method(evidence, method, &arguments, enclosing)
            }
            StaticDispatch::InterfaceSelfMethod { interface, method } => {
                self.resolve_interface_self_method(interface, method, &arguments, enclosing)
            }
            StaticDispatch::InterfaceDefault {
                interface,
                receiver,
                method,
            } => {
                let receiver = enclosing.apply_type(self.semantics.types_mut(), receiver)?;
                if !is_concrete_type(self.semantics.types(), receiver)? {
                    return Err(ConcreteDispatchError::SymbolicInterfaceSelf {
                        interface,
                        receiver,
                    });
                }
                Ok(ResolvedDispatchPlan::Invocation(
                    ResolvedDispatchStep::Direct(ResolvedCallableDispatch {
                        callable: method,
                        generic_arguments: arguments,
                        interface_self: Some((interface, receiver)),
                    }),
                ))
            }
            StaticDispatch::OpaqueMethod { opaque, method } => {
                self.resolve_opaque_method(opaque, method, &arguments, enclosing)
            }
            StaticDispatch::StructuralRequirement { evidence } => {
                self.resolve_structural(evidence, enclosing)
            }
        }
    }

    fn specialize_arguments(
        &mut self,
        arguments: &GenericArguments,
        enclosing: &TypeSubstitution,
    ) -> Result<GenericArguments, ConcreteDispatchError> {
        let specialized = arguments
            .as_slice()
            .iter()
            .map(|argument| {
                enclosing
                    .apply_type(self.semantics.types_mut(), argument.ty())
                    .map(|ty| GenericArgument::new(argument.parameter(), ty))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for argument in &specialized {
            if !is_concrete_type(self.semantics.types(), argument.ty())? {
                return Err(ConcreteDispatchError::SymbolicArgument {
                    parameter: argument.parameter(),
                    ty: argument.ty(),
                });
            }
        }
        GenericArguments::new(specialized)
            .map_err(|duplicate| ConcreteDispatchError::DuplicateGeneric(duplicate.parameter()))
    }

    fn resolve_interface_method(
        &mut self,
        evidence: nocter_model::CapabilityEvidenceId,
        surface: CallableId,
        specialized_arguments: &GenericArguments,
        enclosing: &TypeSubstitution,
    ) -> Result<ResolvedDispatchPlan, ConcreteDispatchError> {
        let requirement = self.evidence_root(evidence)?;
        let predicate = self.normalized_evidence(evidence, enclosing)?;
        let CheckedPredicate::Interface {
            subject,
            application,
            ..
        } = predicate
        else {
            return Err(ConcreteDispatchError::InvalidInterfaceRequirement(
                requirement,
            ));
        };
        if self
            .program
            .graph()
            .interface_capabilities()
            .get(application.interface())
            .is_none_or(|capability| !capability.methods().contains(&surface))
        {
            return Err(ConcreteDispatchError::InvalidInterfaceMethod {
                requirement,
                method: surface,
            });
        }
        self.resolve_interface_application_method(
            subject,
            &application,
            surface,
            specialized_arguments,
            Some(requirement),
        )
    }

    fn resolve_interface_self_method(
        &mut self,
        interface: InterfaceId,
        surface: CallableId,
        specialized_arguments: &GenericArguments,
        enclosing: &TypeSubstitution,
    ) -> Result<ResolvedDispatchPlan, ConcreteDispatchError> {
        let symbolic = self
            .semantics
            .types_mut()
            .intern(TypeKind::InterfaceSelf(interface))
            .map_err(|_| ConcreteDispatchError::InvalidInterfaceSelfMethod {
                interface,
                surface,
            })?;
        let subject = enclosing.apply_type(self.semantics.types_mut(), symbolic)?;
        if !is_concrete_type(self.semantics.types(), subject)? {
            return Err(ConcreteDispatchError::SymbolicInterfaceSelf {
                interface,
                receiver: subject,
            });
        }
        let declaration = self
            .program
            .graph()
            .declarations()
            .interfaces()
            .get(interface)
            .ok_or(ConcreteDispatchError::InvalidInterfaceSelfMethod { interface, surface })?;
        if !declaration.methods().contains(&surface) {
            return Err(ConcreteDispatchError::InvalidInterfaceSelfMethod { interface, surface });
        }
        let application = nocter_declarations::InterfaceApplication::new(
            interface,
            declaration
                .generic_parameters()
                .iter()
                .map(|parameter| {
                    specialized_arguments.get(*parameter).ok_or(
                        ConcreteDispatchError::MissingMethodArgument {
                            method: surface,
                            parameter: *parameter,
                        },
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
        self.resolve_interface_application_method(
            subject,
            &application,
            surface,
            specialized_arguments,
            None,
        )
    }

    fn resolve_interface_application_method(
        &mut self,
        subject: TypeId,
        application: &nocter_declarations::InterfaceApplication,
        surface: CallableId,
        specialized_arguments: &GenericArguments,
        requirement: Option<RequirementId>,
    ) -> Result<ResolvedDispatchPlan, ConcreteDispatchError> {
        let candidates = self
            .evidence()
            .interface_method(subject, application, surface)?;
        let candidate = if let Some(requirement) = requirement {
            exactly_one(candidates, requirement)?
        } else {
            let mut candidates = candidates.into_iter();
            let Some(candidate) = candidates.next() else {
                return Err(ConcreteDispatchError::InvalidInterfaceSelfMethod {
                    interface: application.interface(),
                    surface,
                });
            };
            if candidates.next().is_some() {
                return Err(ConcreteDispatchError::InvalidInterfaceSelfMethod {
                    interface: application.interface(),
                    surface,
                });
            }
            candidate
        };
        let target = candidate.callable();
        let surface_declaration = self
            .program
            .graph()
            .declarations()
            .callables()
            .get(surface)
            .ok_or(ConcreteDispatchError::UnknownCallable(surface))?;
        let target_declaration = self
            .program
            .graph()
            .declarations()
            .callables()
            .get(target)
            .ok_or(ConcreteDispatchError::UnknownCallable(target))?;
        if surface_declaration.generic_parameters().len()
            != target_declaration.generic_parameters().len()
        {
            return Err(ConcreteDispatchError::MethodGenericDomainMismatch { surface, target });
        }
        let mut target_arguments = candidate.generic_arguments().as_slice().to_vec();
        for (source, target_parameter) in surface_declaration
            .generic_parameters()
            .iter()
            .copied()
            .zip(target_declaration.generic_parameters().iter().copied())
        {
            let ty = specialized_arguments.get(source).ok_or(
                ConcreteDispatchError::MissingMethodArgument {
                    method: surface,
                    parameter: source,
                },
            )?;
            target_arguments.push(GenericArgument::new(target_parameter, ty));
        }
        let generic_arguments = GenericArguments::new(target_arguments)
            .map_err(|duplicate| ConcreteDispatchError::DuplicateGeneric(duplicate.parameter()))?;
        Ok(ResolvedDispatchPlan::Invocation(
            ResolvedDispatchStep::Direct(ResolvedCallableDispatch {
                callable: target,
                generic_arguments,
                interface_self: match candidate.dispatch() {
                    StaticDispatch::InterfaceDefault {
                        interface,
                        receiver,
                        ..
                    } => Some((interface, receiver)),
                    StaticDispatch::Direct(_) => None,
                    _ => return Err(ConcreteDispatchError::NonConcreteCandidate),
                },
            }),
        ))
    }

    fn resolve_opaque_method(
        &mut self,
        opaque: TypeId,
        surface: CallableId,
        specialized_arguments: &GenericArguments,
        enclosing: &TypeSubstitution,
    ) -> Result<ResolvedDispatchPlan, ConcreteDispatchError> {
        let specialized = self.specialize_opaque_witness(opaque, enclosing)?;
        let definition = specialized.definition;
        let candidates = self.evidence().interface_method(
            specialized.witness,
            &specialized.application,
            surface,
        )?;
        let [candidate] = candidates.as_slice() else {
            return Err(if candidates.is_empty() {
                ConcreteDispatchError::MissingOpaqueEvidence(definition)
            } else {
                ConcreteDispatchError::AmbiguousOpaqueEvidence(definition)
            });
        };
        let target = candidate.callable();
        let surface_declaration = self
            .program
            .graph()
            .declarations()
            .callables()
            .get(surface)
            .ok_or(ConcreteDispatchError::UnknownCallable(surface))?;
        let target_declaration = self
            .program
            .graph()
            .declarations()
            .callables()
            .get(target)
            .ok_or(ConcreteDispatchError::UnknownCallable(target))?;
        if surface_declaration.generic_parameters().len()
            != target_declaration.generic_parameters().len()
        {
            return Err(ConcreteDispatchError::MethodGenericDomainMismatch { surface, target });
        }
        let mut target_arguments = candidate.generic_arguments().as_slice().to_vec();
        for (source, target_parameter) in surface_declaration
            .generic_parameters()
            .iter()
            .copied()
            .zip(target_declaration.generic_parameters().iter().copied())
        {
            let ty = specialized_arguments.get(source).ok_or(
                ConcreteDispatchError::MissingMethodArgument {
                    method: surface,
                    parameter: source,
                },
            )?;
            target_arguments.push(GenericArgument::new(target_parameter, ty));
        }
        let generic_arguments = GenericArguments::new(target_arguments)
            .map_err(|duplicate| ConcreteDispatchError::DuplicateGeneric(duplicate.parameter()))?;
        let receiver = target_declaration
            .receiver()
            .and_then(|receiver| {
                self.program
                    .graph()
                    .declarations()
                    .parameters()
                    .get(receiver)
            })
            .and_then(|receiver| match receiver.role() {
                ParameterRole::Receiver(capability) => Some(capability),
                ParameterRole::Ordinary { .. } | ParameterRole::ArgumentPack { .. } => None,
            })
            .ok_or(ConcreteDispatchError::InvalidOpaqueType(opaque))?;
        let source = opaque_receiver_type(self.semantics.types_mut(), receiver, opaque)
            .map_err(|_| ConcreteDispatchError::InvalidOpaqueType(opaque))?;
        let receiver_target =
            opaque_receiver_type(self.semantics.types_mut(), receiver, specialized.witness)
                .map_err(|_| ConcreteDispatchError::InvalidOpaqueType(opaque))?;
        Ok(ResolvedDispatchPlan::OpaqueInvocation {
            receiver: ResolvedOpaqueReceiver {
                definition,
                opaque,
                witness: specialized.witness,
                source,
                target: receiver_target,
            },
            operation: ResolvedDispatchStep::Direct(ResolvedCallableDispatch {
                callable: target,
                generic_arguments,
                interface_self: None,
            }),
        })
    }

    fn specialize_opaque_witness(
        &mut self,
        opaque: TypeId,
        enclosing: &TypeSubstitution,
    ) -> Result<SpecializedOpaqueWitness, ConcreteDispatchError> {
        let opaque = enclosing.apply_type(self.semantics.types_mut(), opaque)?;
        let Some(TypeKind::Opaque {
            definition,
            arguments,
        }) = self.semantics.types().get(opaque).cloned()
        else {
            return Err(ConcreteDispatchError::InvalidOpaqueType(opaque));
        };
        let declaration = self
            .program
            .graph()
            .declarations()
            .opaque_types()
            .get(definition)
            .cloned()
            .ok_or(ConcreteDispatchError::InvalidOpaqueType(opaque))?;
        if declaration.generic_parameters().len() != arguments.len() {
            return Err(ConcreteDispatchError::InvalidOpaqueType(opaque));
        }
        let mut substitution = TypeSubstitution::default();
        for (parameter, argument) in declaration
            .generic_parameters()
            .iter()
            .copied()
            .zip(arguments)
        {
            substitution.bind_generic(parameter, argument);
        }
        let witness = self
            .program
            .opaque_witnesses()
            .get(definition)
            .ok_or(ConcreteDispatchError::MissingOpaqueWitness(definition))?;
        let witness = substitution.apply_type(self.semantics.types_mut(), witness)?;
        if !is_concrete_type(self.semantics.types(), witness)? {
            return Err(ConcreteDispatchError::InvalidOpaqueType(opaque));
        }
        let application = nocter_declarations::InterfaceApplication::new(
            declaration.interface().interface(),
            declaration
                .interface()
                .arguments()
                .iter()
                .map(|argument| substitution.apply_type(self.semantics.types_mut(), *argument))
                .collect::<Result<Vec<_>, _>>()?,
        );
        Ok(SpecializedOpaqueWitness {
            definition,
            witness,
            application,
        })
    }

    fn resolve_structural(
        &mut self,
        evidence: nocter_model::CapabilityEvidenceId,
        enclosing: &TypeSubstitution,
    ) -> Result<ResolvedDispatchPlan, ConcreteDispatchError> {
        let requirement = self.evidence_root(evidence)?;
        let predicate = self.normalized_evidence(evidence, enclosing)?;
        match predicate {
            CheckedPredicate::Callable { subject, contract } => {
                Ok(ResolvedDispatchPlan::Invocation(
                    ResolvedDispatchStep::CallableValue { subject, contract },
                ))
            }
            CheckedPredicate::Equality(ty) => {
                self.resolve_comparison(requirement, ty, ComparisonOperation::Equal)
            }
            CheckedPredicate::Ordering(ty) => {
                self.resolve_comparison(requirement, ty, ComparisonOperation::Less)
            }
            CheckedPredicate::Index {
                capability,
                container,
                index,
                result,
            } => self.resolve_index(requirement, capability, container, index, result),
            CheckedPredicate::Coercion { source, target } => {
                self.resolve_coercion(requirement, source, target)
            }
            CheckedPredicate::Expansion {
                capability,
                source,
                result,
            } => self.resolve_expansion(requirement, capability, source, result),
            CheckedPredicate::Interface { .. }
            | CheckedPredicate::Copy(_)
            | CheckedPredicate::BinderRefinement { .. } => {
                Err(ConcreteDispatchError::NonRuntimeRequirement(requirement))
            }
        }
    }

    fn normalized_evidence(
        &mut self,
        evidence: nocter_model::CapabilityEvidenceId,
        substitution: &TypeSubstitution,
    ) -> Result<CheckedPredicate, ConcreteDispatchError> {
        let predicate = self
            .program
            .environment()
            .capability_evidence()
            .get(evidence)
            .ok_or(ConcreteDispatchError::InvalidCapabilityEvidence(evidence))?
            .predicate()
            .clone();
        Ok(substitute_predicate(
            self.semantics.types_mut(),
            substitution,
            &predicate,
        )?)
    }

    fn evidence_root(
        &self,
        evidence: nocter_model::CapabilityEvidenceId,
    ) -> Result<RequirementId, ConcreteDispatchError> {
        self.program
            .environment()
            .capability_evidence()
            .get(evidence)
            .map(crate::body_check::CapabilityEvidence::root)
            .ok_or(ConcreteDispatchError::InvalidCapabilityEvidence(evidence))
    }

    fn resolve_comparison(
        &mut self,
        requirement: RequirementId,
        ty: TypeId,
        operation: ComparisonOperation,
    ) -> Result<ResolvedDispatchPlan, ConcreteDispatchError> {
        let candidates = self.evidence().comparison(ty, ty, operation)?;
        let candidate = exactly_one(candidates, requirement)?;
        let left_coercion = candidate
            .receiver_coercion()
            .map(Self::direct_step)
            .transpose()?;
        let right_coercion = candidate
            .argument_coercion()
            .map(Self::direct_step)
            .transpose()?;
        let operation = match candidate.implementation() {
            ComparisonCandidateImplementation::Primitive => {
                let operand = self
                    .semantics
                    .types_mut()
                    .intern(TypeKind::Borrow {
                        capability: BorrowCapability::Readonly,
                        referent: ty,
                    })
                    .map_err(|_| SubstitutionError::InvalidStore)?;
                ResolvedDispatchStep::Primitive(match operation {
                    ComparisonOperation::Equal => ResolvedPrimitiveDispatch::Equality {
                        subject: ty,
                        operand,
                    },
                    ComparisonOperation::Less => ResolvedPrimitiveDispatch::Ordering {
                        subject: ty,
                        operand,
                    },
                })
            }
            ComparisonCandidateImplementation::Selected(selection) => Self::direct_step(selection)?,
        };
        Ok(ResolvedDispatchPlan::Comparison {
            left_coercion,
            right_coercion,
            operation,
        })
    }

    fn resolve_index(
        &mut self,
        requirement: RequirementId,
        capability: BorrowCapability,
        container: TypeId,
        index: TypeId,
        result: TypeId,
    ) -> Result<ResolvedDispatchPlan, ConcreteDispatchError> {
        let Some((result_capability, referent)) = borrow_result(self.semantics.types(), result)
        else {
            return Err(ConcreteDispatchError::InvalidIndexResult(requirement));
        };
        if result_capability != capability {
            return Err(ConcreteDispatchError::InvalidIndexResult(requirement));
        }
        if let Some(builtin) = builtin_index_result(self.semantics.types(), container, capability)
            && index
                == self
                    .semantics
                    .types()
                    .builtin(nocter_model::BuiltinType::Usize)
            && referent == builtin
        {
            let receiver = self
                .semantics
                .types_mut()
                .intern(TypeKind::Borrow {
                    capability,
                    referent: container,
                })
                .map_err(|_| SubstitutionError::InvalidStore)?;
            return Ok(ResolvedDispatchPlan::Index {
                receiver_coercion: None,
                operation: ResolvedDispatchStep::Primitive(ResolvedPrimitiveDispatch::Index {
                    capability,
                    container,
                    receiver,
                    index,
                    result,
                }),
            });
        }
        let candidates = self
            .evidence()
            .index(container, capability, index, referent)?;
        let candidate = exactly_one(candidates, requirement)?;
        let receiver_coercion = candidate
            .receiver_coercion()
            .map(Self::direct_step)
            .transpose()?;
        let operation = if let Some(operation) = candidate.operation() {
            Self::direct_step(operation)?
        } else {
            let receiver = self
                .semantics
                .types_mut()
                .intern(TypeKind::Borrow {
                    capability,
                    referent: container,
                })
                .map_err(|_| SubstitutionError::InvalidStore)?;
            ResolvedDispatchStep::Primitive(ResolvedPrimitiveDispatch::Index {
                capability,
                container,
                receiver,
                index,
                result,
            })
        };
        Ok(ResolvedDispatchPlan::Index {
            receiver_coercion,
            operation,
        })
    }

    fn resolve_coercion(
        &mut self,
        requirement: RequirementId,
        source: TypeId,
        target: TypeId,
    ) -> Result<ResolvedDispatchPlan, ConcreteDispatchError> {
        let Some((source_capability, source_owner)) = borrow_result(self.semantics.types(), source)
        else {
            return Err(ConcreteDispatchError::InvalidCoercion(requirement));
        };
        let Some((target_capability, target_owner)) = borrow_result(self.semantics.types(), target)
        else {
            return Err(ConcreteDispatchError::InvalidCoercion(requirement));
        };
        if source_owner == target_owner
            && source_capability == BorrowCapability::ReadWrite
            && target_capability == BorrowCapability::Readonly
        {
            return Ok(ResolvedDispatchPlan::Invocation(
                ResolvedDispatchStep::Primitive(ResolvedPrimitiveDispatch::BorrowWeakening {
                    source,
                    target,
                }),
            ));
        }
        let candidates = self.evidence().coercion(
            source_owner,
            source_capability,
            target_capability,
            target_owner,
        )?;
        let candidate = exactly_one(candidates, requirement)?;
        Ok(ResolvedDispatchPlan::Invocation(Self::direct_step(
            candidate.selection(),
        )?))
    }

    fn resolve_expansion(
        &mut self,
        requirement: RequirementId,
        capability: ExpansionCapability,
        source: TypeId,
        result: TypeId,
    ) -> Result<ResolvedDispatchPlan, ConcreteDispatchError> {
        let candidates = self.evidence().expansion(source, capability, result)?;
        let candidate = exactly_one(candidates, requirement)?;
        Ok(ResolvedDispatchPlan::Invocation(Self::direct_step(
            candidate.selection(),
        )?))
    }

    fn evidence(&mut self) -> ConcreteEvidenceAuthority<'_> {
        let (types, copyabilities) = self.semantics.access().into_reasoning_parts();
        ConcreteEvidenceAuthority::new(self.program, types, copyabilities)
    }

    fn direct_step(
        selection: &StaticSelection,
    ) -> Result<ResolvedDispatchStep, ConcreteDispatchError> {
        let StaticDispatch::Direct(callable) = selection.dispatch() else {
            return Err(ConcreteDispatchError::NonConcreteCandidate);
        };
        Ok(ResolvedDispatchStep::Direct(ResolvedCallableDispatch {
            callable,
            generic_arguments: selection.generic_arguments().clone(),
            interface_self: None,
        }))
    }
}

fn exactly_one<T>(
    candidates: Vec<T>,
    requirement: RequirementId,
) -> Result<T, ConcreteDispatchError> {
    let mut candidates = candidates.into_iter();
    let Some(candidate) = candidates.next() else {
        return Err(ConcreteDispatchError::MissingEvidence(requirement));
    };
    if candidates.next().is_some() {
        return Err(ConcreteDispatchError::AmbiguousEvidence(requirement));
    }
    Ok(candidate)
}

fn opaque_receiver_type(
    types: &mut nocter_model::TypeTransaction,
    capability: CallableCapability,
    referent: TypeId,
) -> Result<TypeId, nocter_model::UnknownTypeId> {
    match capability {
        CallableCapability::Owned => Ok(referent),
        CallableCapability::Readonly => types.intern(TypeKind::Borrow {
            capability: BorrowCapability::Readonly,
            referent,
        }),
        CallableCapability::ReadWrite => types.intern(TypeKind::Borrow {
            capability: BorrowCapability::ReadWrite,
            referent,
        }),
    }
}

fn borrow_result(types: &TypeStore, ty: TypeId) -> Option<(BorrowCapability, TypeId)> {
    let nocter_model::TypeKind::Borrow {
        capability,
        referent,
    } = types.get(ty)?
    else {
        return None;
    };
    Some((*capability, *referent))
}

fn builtin_index_result(
    types: &TypeStore,
    target: TypeId,
    capability: BorrowCapability,
) -> Option<TypeId> {
    match types.get(target)? {
        nocter_model::TypeKind::FixedArray { element, .. }
        | nocter_model::TypeKind::Slice(element) => Some(*element),
        nocter_model::TypeKind::Builtin(nocter_model::BuiltinType::Str)
            if capability == BorrowCapability::Readonly =>
        {
            Some(types.builtin(nocter_model::BuiltinType::U8))
        }
        _ => None,
    }
}

/// Failure to convert checked generic dispatch into one exact executable plan.
#[derive(Debug)]
pub enum ConcreteDispatchError {
    UnknownRequirement(RequirementId),
    InvalidCapabilityEvidence(nocter_model::CapabilityEvidenceId),
    UnknownCallable(CallableId),
    InvalidInterfaceRequirement(RequirementId),
    InvalidInterfaceMethod {
        requirement: RequirementId,
        method: CallableId,
    },
    InvalidInterfaceSelfMethod {
        interface: InterfaceId,
        surface: CallableId,
    },
    SymbolicInterfaceSelf {
        interface: InterfaceId,
        receiver: TypeId,
    },
    MethodGenericDomainMismatch {
        surface: CallableId,
        target: CallableId,
    },
    MissingMethodArgument {
        method: CallableId,
        parameter: GenericParameterId,
    },
    InvalidOpaqueType(TypeId),
    MissingOpaqueWitness(nocter_model::OpaqueTypeId),
    MissingOpaqueEvidence(nocter_model::OpaqueTypeId),
    AmbiguousOpaqueEvidence(nocter_model::OpaqueTypeId),
    InvalidIndexResult(RequirementId),
    InvalidCoercion(RequirementId),
    NonRuntimeRequirement(RequirementId),
    MissingEvidence(RequirementId),
    AmbiguousEvidence(RequirementId),
    NonConcreteCandidate,
    SymbolicArgument {
        parameter: GenericParameterId,
        ty: TypeId,
    },
    DuplicateGeneric(GenericParameterId),
    Substitution(SubstitutionError),
    Selection(InstanceSelectionError),
}

impl fmt::Display for ConcreteDispatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRequirement(_) => {
                formatter.write_str("concrete dispatch names an unknown requirement")
            }
            Self::InvalidCapabilityEvidence(_) => {
                formatter.write_str("concrete dispatch names unknown capability evidence")
            }
            Self::UnknownCallable(_) => {
                formatter.write_str("concrete dispatch names an unknown callable")
            }
            Self::InvalidInterfaceRequirement(_) => {
                formatter.write_str("interface dispatch does not name an interface requirement")
            }
            Self::InvalidInterfaceMethod { .. } => {
                formatter.write_str("interface dispatch names a method outside its interface")
            }
            Self::InvalidInterfaceSelfMethod { .. } => formatter
                .write_str("interface default dispatch has no exact concrete method evidence"),
            Self::SymbolicInterfaceSelf { .. } => {
                formatter.write_str("interface default dispatch retains a symbolic Self type")
            }
            Self::MethodGenericDomainMismatch { .. } => formatter
                .write_str("interface and implementation method generic domains do not match"),
            Self::MissingMethodArgument { .. } => {
                formatter.write_str("interface dispatch is missing a method generic argument")
            }
            Self::InvalidOpaqueType(_) => {
                formatter.write_str("opaque dispatch does not name one concrete opaque type")
            }
            Self::MissingOpaqueWitness(_) => {
                formatter.write_str("opaque dispatch has no checked witness")
            }
            Self::MissingOpaqueEvidence(_) => {
                formatter.write_str("opaque witness has no applicable interface evidence")
            }
            Self::AmbiguousOpaqueEvidence(_) => {
                formatter.write_str("opaque witness has ambiguous interface evidence")
            }
            Self::InvalidIndexResult(_) => {
                formatter.write_str("structural index dispatch has an invalid result contract")
            }
            Self::InvalidCoercion(_) => {
                formatter.write_str("structural coercion dispatch has an invalid borrow contract")
            }
            Self::NonRuntimeRequirement(_) => {
                formatter.write_str("static dispatch names a non-runtime requirement")
            }
            Self::MissingEvidence(_) => {
                formatter.write_str("concrete dispatch has no applicable evidence")
            }
            Self::AmbiguousEvidence(_) => {
                formatter.write_str("concrete dispatch has ambiguous applicable evidence")
            }
            Self::NonConcreteCandidate => {
                formatter.write_str("concrete selection produced another symbolic dispatch")
            }
            Self::SymbolicArgument { .. } => {
                formatter.write_str("concrete dispatch retains a symbolic generic argument")
            }
            Self::DuplicateGeneric(_) => {
                formatter.write_str("concrete dispatch bound one generic more than once")
            }
            Self::Substitution(error) => error.fmt(formatter),
            Self::Selection(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConcreteDispatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Substitution(error) => Some(error),
            Self::Selection(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SubstitutionError> for ConcreteDispatchError {
    fn from(error: SubstitutionError) -> Self {
        Self::Substitution(error)
    }
}

impl From<InstanceSelectionError> for ConcreteDispatchError {
    fn from(error: InstanceSelectionError) -> Self {
        Self::Selection(error)
    }
}
