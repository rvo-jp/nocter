use std::collections::HashSet;

use nocter_declarations::{DeclarationGraph, InterfaceApplication};
use nocter_model::{ArgumentPack, CallableContract, InterfaceImplementationId, TypeId, TypeKind};

use super::model::InterfaceImplementationTable;
use super::overlap::match_pattern;
use super::predicate::{CheckedPredicate, RequirementPredicate, substitute_predicate};
use crate::associated_type_resolution::{AssociatedTypeResolutionError, AssociatedTypeResolver};
use crate::type_relations::{SubstitutionError, TypeSubstitution};

/// One explicit interface implementation selected for an exact interface application and subject type.
pub(crate) struct InterfaceImplementationSelection {
    declaration: InterfaceImplementationId,
    substitution: TypeSubstitution,
}

/// Applicability of one concrete associated projection across every application of its owner
/// interface.
pub(crate) enum AssociatedImplementationSelection {
    None,
    Unique(InterfaceImplementationSelection),
    Ambiguous,
}

/// Type-level facts available while proving conditional interface implementations.
///
/// Declaration checking has no closure values. Executable specialization adds the frozen closure
/// table so a concrete anonymous closure can satisfy the same callable predicate already accepted
/// during body checking.
#[derive(Clone, Copy)]
pub(crate) struct CallableProofContext<'program> {
    graph: &'program DeclarationGraph,
    closures: Option<&'program crate::ClosureTable>,
}

impl<'program> CallableProofContext<'program> {
    pub(crate) const fn declarations(graph: &'program DeclarationGraph) -> Self {
        Self {
            graph,
            closures: None,
        }
    }

    pub(crate) const fn executable(
        graph: &'program DeclarationGraph,
        closures: &'program crate::ClosureTable,
    ) -> Self {
        Self {
            graph,
            closures: Some(closures),
        }
    }
}

impl InterfaceImplementationSelection {
    pub(crate) const fn declaration(&self) -> InterfaceImplementationId {
        self.declaration
    }

    pub(crate) const fn substitution(&self) -> &TypeSubstitution {
        &self.substitution
    }
}

pub(crate) fn proves<R: RequirementPredicate>(
    context: CallableProofContext<'_>,
    types: &mut nocter_model::TypeTransaction,
    table: &InterfaceImplementationTable,
    assumptions: &[R],
    intrinsic_facts: &[CheckedPredicate],
    predicate: &CheckedPredicate,
) -> Result<bool, SubstitutionError> {
    Prover::new(context, types, table, assumptions, intrinsic_facts).prove(predicate)
}

/// Selects the explicit interface implementation that proves one exact interface application.
///
/// Lexical assumptions may prove conditional requirements but are not returned as invented
/// interface implementation declarations. Program-wide overlap validation guarantees at most one match.
pub(crate) fn select_interface_implementation<R: RequirementPredicate>(
    context: CallableProofContext<'_>,
    types: &mut nocter_model::TypeTransaction,
    table: &InterfaceImplementationTable,
    assumptions: &[R],
    intrinsic_facts: &[CheckedPredicate],
    subject: TypeId,
    application: &InterfaceApplication,
) -> Result<Option<InterfaceImplementationSelection>, SubstitutionError> {
    let mut prover = Prover::new(context, types, table, assumptions, intrinsic_facts);
    let root = CheckedPredicate::Interface {
        subject,
        application: application.clone(),
        associated_types: Box::new([]),
    };
    if !prover.active.insert(root.clone()) {
        return Ok(None);
    }
    let selected = prover.select_interface(subject, application)?;
    prover.active.remove(&root);
    Ok(selected)
}

/// Selects an implementation for a concrete associated projection without inventing interface
/// arguments that are absent from `Base.Member` syntax.
///
/// Unlike ordinary interface dispatch, this searches every application of the associated type's
/// owner interface. More than one applicable application is therefore an ambiguity even when the
/// program-wide overlap rule permits those applications independently.
pub(crate) fn select_associated_implementation<R: RequirementPredicate>(
    context: CallableProofContext<'_>,
    types: &mut nocter_model::TypeTransaction,
    table: &InterfaceImplementationTable,
    assumptions: &[R],
    intrinsic_facts: &[CheckedPredicate],
    subject: TypeId,
    interface: nocter_model::InterfaceId,
) -> Result<AssociatedImplementationSelection, SubstitutionError> {
    Prover::new(context, types, table, assumptions, intrinsic_facts)
        .select_associated(subject, interface)
}

/// Resolves one associated declaration through an already-selected concrete implementation.
///
/// Selection and binding substitution deliberately remain one authority. Callers must not look up
/// an implementation entry and reconstruct its pattern substitution independently.
pub(crate) fn resolve_selected_associated_type(
    types: &mut nocter_model::TypeTransaction,
    table: &InterfaceImplementationTable,
    selection: &InterfaceImplementationSelection,
    associated: nocter_model::AssociatedTypeId,
) -> Result<TypeId, SubstitutionError> {
    let implementation = table
        .entries()
        .get(&selection.declaration())
        .ok_or(SubstitutionError::InvalidStore)?;
    let bound = implementation
        .associated_type(associated)
        .ok_or(SubstitutionError::InvalidStore)?;
    selection.substitution().apply_type(types, bound)
}

struct Prover<'program, R> {
    context: CallableProofContext<'program>,
    types: &'program mut nocter_model::TypeTransaction,
    table: &'program InterfaceImplementationTable,
    assumptions: &'program [R],
    intrinsic_facts: &'program [CheckedPredicate],
    active: HashSet<CheckedPredicate>,
    proven: HashSet<CheckedPredicate>,
}

impl<'program, R: RequirementPredicate> Prover<'program, R> {
    fn new(
        context: CallableProofContext<'program>,
        types: &'program mut nocter_model::TypeTransaction,
        table: &'program InterfaceImplementationTable,
        assumptions: &'program [R],
        intrinsic_facts: &'program [CheckedPredicate],
    ) -> Self {
        Self {
            context,
            types,
            table,
            assumptions,
            intrinsic_facts,
            active: HashSet::new(),
            proven: HashSet::new(),
        }
    }

    fn prove(&mut self, predicate: &CheckedPredicate) -> Result<bool, SubstitutionError> {
        for assumption in self.assumptions {
            if predicate_implies(self.types, assumption.predicate(), predicate)? {
                return Ok(true);
            }
        }
        for fact in self.intrinsic_facts {
            if predicate_implies(self.types, fact, predicate)? {
                return Ok(true);
            }
        }
        if self.proven.contains(predicate) {
            return Ok(true);
        }
        if !self.active.insert(predicate.clone()) {
            return Ok(false);
        }
        let result = match predicate {
            CheckedPredicate::BinderRefinement {
                binder,
                replacement,
            } => binder == replacement,
            CheckedPredicate::Callable {
                subject,
                contract: expected,
            } => {
                let Some(expected) = self.reduce_callable_contract(expected)? else {
                    self.active.remove(predicate);
                    return Ok(false);
                };
                let normalized = CheckedPredicate::Callable {
                    subject: *subject,
                    contract: expected.clone(),
                };
                self.assumptions
                    .iter()
                    .any(|actual| actual.predicate() == &normalized)
                    || self.intrinsic_facts.contains(&normalized)
                    || matches!(
                        self.types.get(*subject),
                        Some(TypeKind::Callable(actual))
                            if !actual.is_erased() && actual.contract() == &expected
                    )
                    || self.concrete_closure_satisfies(*subject, &expected)?
            }
            CheckedPredicate::Interface {
                subject,
                application,
                associated_types,
            } => match self.select_interface(*subject, application)? {
                None => false,
                Some(selection) => {
                    let implementation = self
                        .table
                        .entries()
                        .get(&selection.declaration())
                        .ok_or(SubstitutionError::InvalidStore)?;
                    let mut satisfied = true;
                    for binding in associated_types {
                        let Some(actual) = implementation
                            .associated_types()
                            .iter()
                            .find(|actual| actual.declaration() == binding.declaration())
                        else {
                            satisfied = false;
                            break;
                        };
                        if selection
                            .substitution()
                            .apply_type(self.types, actual.ty())?
                            != binding.ty()
                        {
                            satisfied = false;
                            break;
                        }
                    }
                    satisfied
                }
            },
            CheckedPredicate::Copy(_)
            | CheckedPredicate::Equality { .. }
            | CheckedPredicate::Ordering { .. }
            | CheckedPredicate::Index { .. }
            | CheckedPredicate::Coercion { .. }
            | CheckedPredicate::Expansion { .. } => false,
        };
        self.active.remove(predicate);
        if result {
            self.proven.insert(predicate.clone());
        }
        Ok(result)
    }

    fn reduce_callable_contract(
        &mut self,
        contract: &CallableContract,
    ) -> Result<Option<CallableContract>, SubstitutionError> {
        let resolver = AssociatedTypeResolver::new(
            self.context.graph,
            self.table,
            self.assumptions,
            self.intrinsic_facts,
            self.context.closures,
        );
        let mut reduce = |ty| match resolver.reduce(self.types, ty) {
            Ok(reduced) => Ok(Some(reduced)),
            Err(
                AssociatedTypeResolutionError::UnavailableImplementation { .. }
                | AssociatedTypeResolutionError::AmbiguousImplementation { .. },
            ) => Ok(None),
            Err(_) => Err(SubstitutionError::InvalidStore),
        };
        let mut parameters = Vec::with_capacity(contract.parameters().len());
        for parameter in contract.parameters() {
            let Some(parameter) = reduce(*parameter)? else {
                return Ok(None);
            };
            parameters.push(parameter);
        }
        let pack = match contract.pack() {
            Some(ArgumentPack::Values(element)) => match reduce(element)? {
                Some(element) => Some(ArgumentPack::Values(element)),
                None => return Ok(None),
            },
            Some(ArgumentPack::Keyed { key, value }) => {
                let Some(key) = reduce(key)? else {
                    return Ok(None);
                };
                let Some(value) = reduce(value)? else {
                    return Ok(None);
                };
                Some(ArgumentPack::Keyed { key, value })
            }
            None => None,
        };
        let Some(result) = reduce(contract.result())? else {
            return Ok(None);
        };
        CallableContract::new_with_input_provenance(
            contract.capability(),
            contract.guarantees(),
            parameters,
            pack,
            contract.input_provenance().clone(),
            result,
            contract.provenance().clone(),
        )
        .map(Some)
        .map_err(|_| SubstitutionError::InvalidStore)
    }

    fn concrete_closure_satisfies(
        &mut self,
        subject: TypeId,
        expected: &CallableContract,
    ) -> Result<bool, SubstitutionError> {
        let Some(closures) = self.context.closures else {
            return Ok(false);
        };
        let Some(TypeKind::Closure {
            definition,
            arguments,
        }) = self.types.get(subject).cloned()
        else {
            return Ok(false);
        };
        let Some(definition) = closures.get(definition) else {
            return Ok(false);
        };
        let domain = self
            .context
            .graph
            .declarations()
            .body_generic_domain(definition.owner())
            .ok_or(SubstitutionError::InvalidStore)?;
        if domain.len() != arguments.len() {
            return Err(SubstitutionError::InvalidStore);
        }
        let mut substitution = TypeSubstitution::default();
        for (parameter, argument) in domain.iter().copied().zip(arguments.iter().copied()) {
            substitution.bind_value(parameter, argument);
        }
        let parameters = definition
            .signature()
            .parameter_types()
            .map(|parameter| substitution.apply_type(self.types, parameter))
            .collect::<Result<Vec<_>, _>>()?;
        let result = substitution.apply_type(self.types, definition.signature().result())?;
        Ok(expected
            .capability()
            .permits(definition.signature().capability())
            && expected.pack().is_none()
            && expected.parameters() == parameters
            && expected.result() == result)
    }

    fn select_interface(
        &mut self,
        subject: TypeId,
        application: &InterfaceApplication,
    ) -> Result<Option<InterfaceImplementationSelection>, SubstitutionError> {
        let candidates = self.table.candidates(application.interface()).to_vec();
        for declaration in candidates {
            let interface_implementation = self
                .table
                .entries()
                .get(&declaration)
                .ok_or(SubstitutionError::InvalidStore)?;
            let Some(matched) = match_pattern(
                self.types,
                interface_implementation.interface(),
                interface_implementation.target(),
                application,
                subject,
            )?
            else {
                continue;
            };
            let mut substitution = TypeSubstitution::default();
            for refinement in interface_implementation.refinements() {
                substitution.bind_value(refinement.parameter(), refinement.value());
            }
            substitution.extend(&matched);
            let requirements = interface_implementation.requirements().to_vec();
            for requirement in requirements {
                let predicate =
                    substitute_predicate(self.types, &substitution, requirement.predicate())?;
                if !self.prove(&predicate)? {
                    return Ok(None);
                }
            }
            return Ok(Some(InterfaceImplementationSelection {
                declaration,
                substitution,
            }));
        }
        Ok(None)
    }

    fn select_associated(
        &mut self,
        subject: TypeId,
        interface: nocter_model::InterfaceId,
    ) -> Result<AssociatedImplementationSelection, SubstitutionError> {
        let candidates = self.table.candidates(interface).to_vec();
        let mut selected = None;
        for declaration in candidates {
            let implementation = self
                .table
                .entries()
                .get(&declaration)
                .ok_or(SubstitutionError::InvalidStore)?;
            let application = implementation.interface().clone();
            let target = implementation.target();
            let refinements = implementation.refinements().to_vec();
            let requirements = implementation.requirements().to_vec();
            let Some(matched) =
                match_pattern(self.types, &application, target, &application, subject)?
            else {
                continue;
            };
            let mut substitution = TypeSubstitution::default();
            for refinement in refinements {
                substitution.bind_value(refinement.parameter(), refinement.value());
            }
            substitution.extend(&matched);
            let mut applicable = true;
            for requirement in requirements {
                let predicate =
                    substitute_predicate(self.types, &substitution, requirement.predicate())?;
                if !self.prove(&predicate)? {
                    applicable = false;
                    break;
                }
            }
            if !applicable {
                continue;
            }
            let selection = InterfaceImplementationSelection {
                declaration,
                substitution,
            };
            if selected.replace(selection).is_some() {
                return Ok(AssociatedImplementationSelection::Ambiguous);
            }
        }
        Ok(selected.map_or(
            AssociatedImplementationSelection::None,
            AssociatedImplementationSelection::Unique,
        ))
    }
}

pub(crate) fn predicate_implies(
    types: &mut nocter_model::TypeTransaction,
    actual: &CheckedPredicate,
    expected: &CheckedPredicate,
) -> Result<bool, SubstitutionError> {
    if let Some(implies) = actual.structural_implies(expected) {
        return Ok(implies);
    }
    Ok(match (actual, expected) {
        (
            CheckedPredicate::Interface {
                subject: actual_subject,
                application: actual_application,
                associated_types: actual_bindings,
            },
            CheckedPredicate::Interface {
                subject: expected_subject,
                application: expected_application,
                associated_types: expected_bindings,
            },
        ) => {
            if actual_subject != expected_subject || actual_application != expected_application {
                false
            } else {
                let mut satisfied = true;
                for expected in expected_bindings {
                    if actual_bindings.iter().any(|actual| actual == expected) {
                        continue;
                    }
                    let projection = types
                        .intern(TypeKind::AssociatedProjection {
                            base: *expected_subject,
                            associated: expected.declaration(),
                        })
                        .map_err(|_| SubstitutionError::InvalidStore)?;
                    if projection != expected.ty() {
                        satisfied = false;
                        break;
                    }
                }
                satisfied
            }
        }
        _ => actual == expected,
    })
}
