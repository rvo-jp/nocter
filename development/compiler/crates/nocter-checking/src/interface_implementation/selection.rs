use std::collections::HashSet;

use nocter_declarations::InterfaceApplication;
use nocter_model::{InterfaceImplementationId, TypeId, TypeKind, TypeStore};

use super::model::InterfaceImplementationTable;
use super::overlap::match_pattern;
use super::predicate::{CheckedPredicate, CheckedRequirement, substitute_predicate};
use crate::type_relations::{SubstitutionError, TypeSubstitution};

/// One explicit interface implementation selected for an exact interface application and subject type.
pub(crate) struct InterfaceImplementationSelection {
    declaration: InterfaceImplementationId,
    substitution: TypeSubstitution,
}

/// Applicability of one concrete associated projection across every application of its owner
/// interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AssociatedImplementationSelection {
    None,
    Unique(InterfaceImplementationId),
    Ambiguous,
}

impl InterfaceImplementationSelection {
    pub(crate) const fn declaration(&self) -> InterfaceImplementationId {
        self.declaration
    }

    pub(crate) const fn substitution(&self) -> &TypeSubstitution {
        &self.substitution
    }
}

pub(crate) fn proves(
    types: &mut TypeStore,
    table: &InterfaceImplementationTable,
    assumptions: &[CheckedRequirement],
    intrinsic_facts: &[CheckedPredicate],
    predicate: &CheckedPredicate,
) -> Result<bool, SubstitutionError> {
    Prover::new(types, table, assumptions, intrinsic_facts).prove(predicate)
}

/// Selects the explicit interface implementation that proves one exact interface application.
///
/// Lexical assumptions may prove conditional requirements but are not returned as invented
/// interface implementation declarations. Program-wide overlap validation guarantees at most one match.
pub(crate) fn select_interface_implementation(
    types: &mut TypeStore,
    table: &InterfaceImplementationTable,
    assumptions: &[CheckedRequirement],
    intrinsic_facts: &[CheckedPredicate],
    subject: TypeId,
    application: &InterfaceApplication,
) -> Result<Option<InterfaceImplementationSelection>, SubstitutionError> {
    let mut prover = Prover::new(types, table, assumptions, intrinsic_facts);
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
pub(crate) fn select_associated_implementation(
    types: &mut TypeStore,
    table: &InterfaceImplementationTable,
    assumptions: &[CheckedRequirement],
    intrinsic_facts: &[CheckedPredicate],
    subject: TypeId,
    interface: nocter_model::InterfaceId,
) -> Result<AssociatedImplementationSelection, SubstitutionError> {
    Prover::new(types, table, assumptions, intrinsic_facts).select_associated(subject, interface)
}

struct Prover<'program> {
    types: &'program mut TypeStore,
    table: &'program InterfaceImplementationTable,
    assumptions: &'program [CheckedRequirement],
    intrinsic_facts: &'program [CheckedPredicate],
    active: HashSet<CheckedPredicate>,
    proven: HashSet<CheckedPredicate>,
}

impl<'program> Prover<'program> {
    fn new(
        types: &'program mut TypeStore,
        table: &'program InterfaceImplementationTable,
        assumptions: &'program [CheckedRequirement],
        intrinsic_facts: &'program [CheckedPredicate],
    ) -> Self {
        Self {
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
            } => matches!(
                self.types.get(*subject),
                Some(TypeKind::Callable(actual)) if actual == expected
            ),
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
            | CheckedPredicate::Equality(_)
            | CheckedPredicate::Ordering(_)
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
                substitution.bind_generic(refinement.parameter(), refinement.ty());
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
                substitution.bind_generic(refinement.parameter(), refinement.ty());
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
            if selected.replace(declaration).is_some() {
                return Ok(AssociatedImplementationSelection::Ambiguous);
            }
        }
        Ok(selected.map_or(
            AssociatedImplementationSelection::None,
            AssociatedImplementationSelection::Unique,
        ))
    }
}

fn predicate_implies(
    types: &mut TypeStore,
    actual: &CheckedPredicate,
    expected: &CheckedPredicate,
) -> Result<bool, SubstitutionError> {
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
