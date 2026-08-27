use std::collections::{BTreeMap, HashSet};
use std::fmt;

use nocter_declarations::{
    AssociatedTypeBinding, CallableDeclaration, CallableProvenanceContract, DeclarationGraph,
    InterfaceApplication, ProvenanceOrigin,
};
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{
    ArenaBuilder, CallableId, GenericParameterId, InterfaceId, InterfaceImplementationId,
    ParameterId, TypeKind,
};
use nocter_source_index::{DiagnosticOrigins, SemanticEntity, SourceOrigin};

use super::diagnostic;
use super::model::{
    CheckedInterfaceImplementation, InterfaceImplementationInputCorrespondence,
    InterfaceImplementationMethod, InterfaceImplementationTable, MethodSelection,
};
use super::overlap::patterns_overlap;
use super::predicate::{CheckedRequirement, normalize_requirements};
use super::required_method::RequiredInterfaceImplementationMethod;
use super::validate::validate_associated_bounds;
use crate::declaration_patterns::DeclarationPatternTable;
use crate::type_relations::{SubstitutionError, TypeSubstitution};

/// Authored interface implementation failure or an inconsistent semantic boundary.
#[derive(Debug)]
pub enum InterfaceImplementationBuildError {
    Rule {
        diagnostic: Box<SourceDiagnostic>,
        missing_methods: Option<Box<MissingInterfaceImplementationMethods>>,
    },
    Internal(InterfaceImplementationInternalError),
}

impl InterfaceImplementationBuildError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule { diagnostic, .. } => Some(diagnostic),
            Self::Internal(_) => None,
        }
    }

    #[must_use]
    #[cfg(test)]
    pub(crate) fn missing_methods(&self) -> Option<&MissingInterfaceImplementationMethods> {
        match self {
            Self::Rule {
                missing_methods, ..
            } => missing_methods.as_deref(),
            Self::Internal(_) => None,
        }
    }

    /// Removes editor repair evidence while preserving the diagnostic failure.
    #[must_use]
    pub(crate) fn take_missing_methods(
        &mut self,
    ) -> Option<Box<MissingInterfaceImplementationMethods>> {
        match self {
            Self::Rule {
                missing_methods, ..
            } => missing_methods.take(),
            Self::Internal(_) => None,
        }
    }
}

impl fmt::Display for InterfaceImplementationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule { diagnostic, .. } => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InterfaceImplementationBuildError {}

impl From<SourceDiagnostic> for InterfaceImplementationBuildError {
    fn from(diagnostic: SourceDiagnostic) -> Self {
        Self::Rule {
            diagnostic: Box::new(diagnostic),
            missing_methods: None,
        }
    }
}

/// Exact specialized signatures missing from one interface implementation declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingInterfaceImplementationMethods {
    interface_implementation: InterfaceImplementationId,
    required: Box<[RequiredInterfaceImplementationMethod]>,
}

impl MissingInterfaceImplementationMethods {
    #[must_use]
    pub(super) fn new(
        interface_implementation: InterfaceImplementationId,
        required: impl Into<Box<[RequiredInterfaceImplementationMethod]>>,
    ) -> Self {
        Self {
            interface_implementation,
            required: required.into(),
        }
    }

    #[must_use]
    pub const fn interface_implementation(&self) -> InterfaceImplementationId {
        self.interface_implementation
    }

    #[must_use]
    pub const fn required(&self) -> &[RequiredInterfaceImplementationMethod] {
        &self.required
    }
}

impl From<InterfaceImplementationInternalError> for InterfaceImplementationBuildError {
    fn from(error: InterfaceImplementationInternalError) -> Self {
        Self::Internal(error)
    }
}

impl From<SubstitutionError> for InterfaceImplementationBuildError {
    fn from(error: SubstitutionError) -> Self {
        Self::Internal(InterfaceImplementationInternalError::Substitution(error))
    }
}

impl From<SubstitutionError> for InterfaceImplementationInternalError {
    fn from(error: SubstitutionError) -> Self {
        Self::Substitution(error)
    }
}

#[derive(Debug)]
pub enum InterfaceImplementationInternalError {
    MissingInterface(InterfaceId),
    MissingInterfaceImplementation(nocter_model::InterfaceImplementationId),
    MissingAssociatedType(nocter_model::AssociatedTypeId),
    MissingCallable(CallableId),
    MissingParameter(ParameterId),
    MissingSource(SemanticEntity),
    InvalidGenericType(GenericParameterId),
    InvalidInterfaceMethodSet(InterfaceId),
    Substitution(SubstitutionError),
}

impl fmt::Display for InterfaceImplementationInternalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInterface(interface) => {
                write!(
                    formatter,
                    "missing interface_implementation interface {interface:?}"
                )
            }
            Self::MissingInterfaceImplementation(interface_implementation) => {
                write!(
                    formatter,
                    "missing interface_implementation {interface_implementation:?}"
                )
            }
            Self::MissingAssociatedType(associated) => {
                write!(formatter, "missing associated type {associated:?}")
            }
            Self::MissingCallable(callable) => write!(formatter, "missing callable {callable:?}"),
            Self::MissingParameter(parameter) => {
                write!(formatter, "missing callable parameter {parameter:?}")
            }
            Self::MissingSource(entity) => write!(formatter, "missing source for {entity:?}"),
            Self::InvalidGenericType(parameter) => {
                write!(formatter, "cannot intern generic type {parameter:?}")
            }
            Self::InvalidInterfaceMethodSet(interface) => {
                write!(
                    formatter,
                    "interface {interface:?} has duplicate method names"
                )
            }
            Self::Substitution(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InterfaceImplementationInternalError {}

/// Builds the sole program-wide interface implementation dispatch table.
///
/// The declaration graph and type store must come from one consumed `DeclarationProgram`.
/// Signature comparison interns specialized expected types into that same store.
///
/// # Errors
///
/// Returns a source-backed error for missing, extra, incompatible, or overlapping interface implementation
/// declarations, and an internal error for an inconsistent declaration/source graph.
#[cfg(test)]
pub(super) fn build_interface_implementation_table(
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    source_index: DiagnosticOrigins<'_>,
) -> Result<InterfaceImplementationTable, InterfaceImplementationBuildError> {
    let patterns = DeclarationPatternTable::build(graph, types)?;
    let operations = crate::admitted_operations::AdmittedOperations::all(graph);
    build_interface_implementation_table_from_ids(
        graph,
        types,
        source_index,
        &patterns,
        operations.interface_implementations(),
    )
}

pub(crate) fn build_interface_implementation_table_from_ids(
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    source_index: DiagnosticOrigins<'_>,
    patterns: &DeclarationPatternTable,
    interface_implementations: &[InterfaceImplementationId],
) -> Result<InterfaceImplementationTable, InterfaceImplementationBuildError> {
    let declarations = graph.declarations();
    let mut entries = BTreeMap::new();
    let mut by_interface = vec![Vec::new(); declarations.interfaces().len()];
    let mut preceding_patterns = BTreeMap::<InterfaceId, Vec<_>>::new();

    for id in interface_implementations {
        let id = *id;
        let interface_implementation = declarations
            .interface_implementations()
            .get(id)
            .ok_or(InterfaceImplementationInternalError::MissingInterfaceImplementation(id))?;
        let pattern = patterns
            .interface_implementation(id)
            .ok_or(InterfaceImplementationInternalError::MissingInterfaceImplementation(id))?;
        let normalized_interface = pattern.interface().clone();
        let normalized_target = pattern.target();
        let interface_id = normalized_interface.interface();
        let interface = declarations.interfaces().get(interface_id).ok_or(
            InterfaceImplementationInternalError::MissingInterface(interface_id),
        )?;
        let owner = declarations
            .instances()
            .get(interface_implementation.owner())
            .ok_or(InterfaceImplementationInternalError::MissingInterfaceImplementation(id))?;
        record_nonoverlapping_pattern(
            types,
            source_index,
            &mut preceding_patterns,
            InterfaceImplementationPattern {
                site: interface_implementation.site(),
                interface: normalized_interface.clone(),
                target: normalized_target,
            },
        )?;

        let associated_types = pattern.associated_types();
        let substitution = interface_implementation_substitution(
            graph,
            &normalized_interface,
            normalized_target,
            associated_types,
        )?;
        let methods = select_methods(MethodSelectionInput {
            graph,
            types,
            source_index,
            interface_id,
            interface_methods: interface.methods(),
            implementation_methods: owner.members(),
            expected_substitution: &substitution,
            actual_substitution: pattern.substitution(),
            interface_implementation_site: interface_implementation.site(),
            interface_implementation: id,
        })?;
        let previous = entries.insert(
            id,
            CheckedInterfaceImplementation::new(
                normalized_interface,
                normalized_target,
                owner.generic_parameters(),
                pattern.lexical().refinements().to_vec(),
                pattern.lexical().requirements().to_vec(),
                associated_types.to_vec(),
                methods,
            ),
        );
        debug_assert!(previous.is_none());
        by_interface[interface_id_index(interface_id, declarations.interfaces())?].push(id);
    }

    let mut by_interface_arena = ArenaBuilder::new();
    for candidates in by_interface {
        by_interface_arena.insert(candidates.into_boxed_slice());
    }
    let table = InterfaceImplementationTable::new(
        entries,
        by_interface_arena.finish(),
        method_interface_index(graph)?,
    );
    validate_associated_bounds(graph, types, source_index, &table)?;
    Ok(table)
}

fn method_interface_index(
    graph: &DeclarationGraph,
) -> Result<BTreeMap<nocter_model::Symbol, Box<[InterfaceId]>>, InterfaceImplementationInternalError>
{
    let declarations = graph.declarations();
    let mut by_method = BTreeMap::<_, Vec<_>>::new();
    for (interface_id, interface) in declarations.interfaces().iter() {
        for method in interface.methods() {
            let callable = declarations.callables().get(*method).ok_or(
                InterfaceImplementationInternalError::MissingCallable(*method),
            )?;
            let name =
                callable
                    .name()
                    .ok_or(InterfaceImplementationInternalError::MissingCallable(
                        *method,
                    ))?;
            let interfaces = by_method.entry(name).or_default();
            if interfaces.last() != Some(&interface_id) {
                interfaces.push(interface_id);
            }
        }
    }
    Ok(by_method
        .into_iter()
        .map(|(name, interfaces)| (name, interfaces.into_boxed_slice()))
        .collect())
}

struct InterfaceImplementationPattern {
    site: nocter_model::DeclarationSiteId,
    interface: InterfaceApplication,
    target: nocter_model::TypeId,
}

type InterfaceImplementationPatterns = BTreeMap<InterfaceId, Vec<InterfaceImplementationPattern>>;

fn record_nonoverlapping_pattern(
    types: &mut nocter_model::TypeTransaction,
    source_index: DiagnosticOrigins<'_>,
    preceding_patterns: &mut InterfaceImplementationPatterns,
    current: InterfaceImplementationPattern,
) -> Result<(), InterfaceImplementationBuildError> {
    if let Some(preceding) = preceding_patterns.get(&current.interface.interface()) {
        for previous in preceding {
            if patterns_overlap(
                types,
                &previous.interface,
                previous.target,
                &current.interface,
                current.target,
            )? {
                return Err(diagnostic::overlapping(
                    site_origin(source_index, current.site)?,
                    site_origin(source_index, previous.site)?,
                )
                .into());
            }
        }
    }
    preceding_patterns
        .entry(current.interface.interface())
        .or_default()
        .push(current);
    Ok(())
}

struct MethodSelectionInput<'program> {
    graph: &'program DeclarationGraph,
    types: &'program mut nocter_model::TypeTransaction,
    source_index: DiagnosticOrigins<'program>,
    interface_id: InterfaceId,
    interface_methods: &'program [CallableId],
    implementation_methods: &'program [CallableId],
    expected_substitution: &'program TypeSubstitution,
    actual_substitution: &'program TypeSubstitution,
    interface_implementation_site: nocter_model::DeclarationSiteId,
    interface_implementation: InterfaceImplementationId,
}

fn select_methods(
    input: MethodSelectionInput<'_>,
) -> Result<Vec<InterfaceImplementationMethod>, InterfaceImplementationBuildError> {
    let MethodSelectionInput {
        graph,
        types,
        source_index,
        interface_id,
        interface_methods,
        implementation_methods,
        expected_substitution,
        actual_substitution,
        interface_implementation_site,
        interface_implementation,
    } = input;
    let declarations = graph.declarations();
    let interface_by_name = interface_method_index(graph, interface_id, interface_methods)?;
    let implementation_by_name = implementation_method_index(
        graph,
        source_index,
        interface_id,
        implementation_methods,
        &interface_by_name,
    )?;

    let mut selected = Vec::with_capacity(interface_methods.len());
    let mut missing = Vec::new();
    for interface_method in interface_methods {
        let expected = declarations.callables().get(*interface_method).ok_or(
            InterfaceImplementationInternalError::MissingCallable(*interface_method),
        )?;
        let name = expected
            .name()
            .ok_or(InterfaceImplementationInternalError::MissingCallable(
                *interface_method,
            ))?;
        let (selection, input_correspondence) = if let Some(implementation) =
            implementation_by_name.get(&name).copied()
        {
            let actual = declarations.callables().get(implementation).ok_or(
                InterfaceImplementationInternalError::MissingCallable(implementation),
            )?;
            let Some(input_correspondence) = compatible_signature(
                graph,
                types,
                expected,
                actual,
                expected_substitution,
                actual_substitution,
            )?
            else {
                return Err(diagnostic::incompatible_method(
                    site_origin(source_index, actual.site())?,
                    site_origin(source_index, expected.site())?,
                )
                .into());
            };
            (
                MethodSelection::Implementation(implementation),
                input_correspondence,
            )
        } else if expected.body().is_some() {
            (
                MethodSelection::Default(*interface_method),
                input_correspondence(expected, expected).ok_or(
                    InterfaceImplementationInternalError::InvalidInterfaceMethodSet(interface_id),
                )?,
            )
        } else {
            missing.push(RequiredInterfaceImplementationMethod::build(
                graph,
                types,
                interface_implementation,
                *interface_method,
                expected,
                expected_substitution,
            )?);
            continue;
        };
        selected.push(InterfaceImplementationMethod::new(
            *interface_method,
            selection,
            input_correspondence,
        ));
    }
    if !missing.is_empty() {
        return Err(missing_methods_error(
            graph,
            source_index,
            interface_implementation_site,
            interface_implementation,
            missing,
        )?);
    }
    selected.sort_unstable_by_key(InterfaceImplementationMethod::interface_method);
    Ok(selected)
}

fn interface_method_index(
    graph: &DeclarationGraph,
    interface: InterfaceId,
    methods: &[CallableId],
) -> Result<BTreeMap<nocter_model::Symbol, CallableId>, InterfaceImplementationInternalError> {
    let mut by_name = BTreeMap::new();
    for method in methods {
        let callable = graph.declarations().callables().get(*method).ok_or(
            InterfaceImplementationInternalError::MissingCallable(*method),
        )?;
        let name = callable
            .name()
            .ok_or(InterfaceImplementationInternalError::MissingCallable(
                *method,
            ))?;
        if by_name.insert(name, *method).is_some() {
            return Err(InterfaceImplementationInternalError::InvalidInterfaceMethodSet(interface));
        }
    }
    Ok(by_name)
}

fn implementation_method_index(
    graph: &DeclarationGraph,
    source_index: DiagnosticOrigins<'_>,
    interface: InterfaceId,
    methods: &[CallableId],
    expected: &BTreeMap<nocter_model::Symbol, CallableId>,
) -> Result<BTreeMap<nocter_model::Symbol, CallableId>, InterfaceImplementationBuildError> {
    let declarations = graph.declarations();
    let interface_site = declarations
        .interfaces()
        .get(interface)
        .ok_or(InterfaceImplementationInternalError::MissingInterface(
            interface,
        ))?
        .site();
    let mut by_name = BTreeMap::new();
    for method in methods {
        let callable = declarations.callables().get(*method).ok_or(
            InterfaceImplementationInternalError::MissingCallable(*method),
        )?;
        if callable.kind() != nocter_declarations::CallableKind::Method {
            continue;
        }
        let name = callable
            .name()
            .ok_or(InterfaceImplementationInternalError::MissingCallable(
                *method,
            ))?;
        if !expected.contains_key(&name) {
            continue;
        }
        if by_name.insert(name, *method).is_some() {
            return Err(diagnostic::extra_method(
                site_origin(source_index, callable.site())?,
                site_origin(source_index, interface_site)?,
            )
            .into());
        }
    }
    Ok(by_name)
}

fn missing_methods_error(
    graph: &DeclarationGraph,
    source_index: DiagnosticOrigins<'_>,
    interface_implementation_site: nocter_model::DeclarationSiteId,
    interface_implementation: InterfaceImplementationId,
    missing: Vec<RequiredInterfaceImplementationMethod>,
) -> Result<InterfaceImplementationBuildError, InterfaceImplementationInternalError> {
    let first = missing
        .first()
        .expect("missing interface_implementation repair is never empty");
    let expected = graph
        .declarations()
        .callables()
        .get(first.interface_method())
        .ok_or(InterfaceImplementationInternalError::MissingCallable(
            first.interface_method(),
        ))?;
    Ok(InterfaceImplementationBuildError::Rule {
        diagnostic: Box::new(diagnostic::missing_method(
            site_origin(source_index, interface_implementation_site)?,
            site_origin(source_index, expected.site())?,
        )),
        missing_methods: Some(Box::new(MissingInterfaceImplementationMethods::new(
            interface_implementation,
            missing,
        ))),
    })
}

fn compatible_signature(
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    expected: &CallableDeclaration,
    actual: &CallableDeclaration,
    owner_substitution: &TypeSubstitution,
    actual_substitution: &TypeSubstitution,
) -> Result<Option<InterfaceImplementationInputCorrespondence>, InterfaceImplementationBuildError> {
    if expected.kind() != actual.kind()
        || expected.parameters().len() != actual.parameters().len()
        || expected.generic_parameters().len() != actual.generic_parameters().len()
    {
        return Ok(None);
    }
    let Some(input_correspondence) = input_correspondence(expected, actual) else {
        return Ok(None);
    };
    let mut substitution = owner_substitution.clone();
    for (expected, actual) in expected
        .generic_parameters()
        .iter()
        .zip(actual.generic_parameters())
    {
        let ty = types
            .intern(TypeKind::GenericParameter(*actual))
            .map_err(|_| InterfaceImplementationInternalError::InvalidGenericType(*actual))?;
        substitution.bind_generic(*expected, ty);
    }
    if receiver_capability(graph, expected.receiver())?
        != receiver_capability(graph, actual.receiver())?
    {
        return Ok(None);
    }
    for (expected, actual) in expected.parameters().iter().zip(actual.parameters()) {
        let (expected_type, expected_pack) = parameter_contract(graph, *expected)?;
        let (actual_type, actual_pack) = parameter_contract(graph, *actual)?;
        if expected_pack != actual_pack
            || substitution.apply_type(types, expected_type)?
                != actual_substitution.apply_type(types, actual_type)?
        {
            return Ok(None);
        }
    }
    if substitution.apply_type(types, expected.result())?
        != actual_substitution.apply_type(types, actual.result())?
    {
        return Ok(None);
    }
    let expected_requirements =
        normalize_requirements(graph, types, &substitution, expected.requirements())?;
    let actual_requirements =
        normalize_requirements(graph, types, actual_substitution, actual.requirements())?;
    if !same_predicates(&expected_requirements, &actual_requirements) {
        return Ok(None);
    }
    if compatible_provenance(expected, actual, &input_correspondence) {
        Ok(Some(input_correspondence))
    } else {
        Ok(None)
    }
}

fn interface_implementation_substitution(
    graph: &DeclarationGraph,
    normalized_interface: &InterfaceApplication,
    normalized_target: nocter_model::TypeId,
    associated_types: &[AssociatedTypeBinding],
) -> Result<TypeSubstitution, InterfaceImplementationBuildError> {
    let interface_id = normalized_interface.interface();
    let interface = graph.declarations().interfaces().get(interface_id).ok_or(
        InterfaceImplementationInternalError::MissingInterface(interface_id),
    )?;
    let mut substitution = TypeSubstitution::default();
    substitution.set_interface_self(interface_id, normalized_target);
    for (parameter, argument) in interface
        .generic_parameters()
        .iter()
        .zip(normalized_interface.arguments())
    {
        substitution.bind_generic(*parameter, *argument);
    }
    for binding in associated_types {
        substitution.bind_associated(binding.declaration(), binding.ty());
    }
    Ok(substitution)
}

fn same_predicates(left: &[CheckedRequirement], right: &[CheckedRequirement]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut matched = HashSet::new();
    left.iter().all(|left| {
        right
            .iter()
            .enumerate()
            .find(|(index, right)| {
                !matched.contains(index) && left.predicate() == right.predicate()
            })
            .is_some_and(|(index, _)| matched.insert(index))
    })
}

fn compatible_provenance(
    expected_declaration: &CallableDeclaration,
    actual_declaration: &CallableDeclaration,
    input_correspondence: &[(ProvenanceOrigin, ProvenanceOrigin)],
) -> bool {
    let (
        CallableProvenanceContract::Declared(expected_contract),
        CallableProvenanceContract::Declared(actual_contract),
    ) = (
        expected_declaration.provenance(),
        actual_declaration.provenance(),
    )
    else {
        return true;
    };
    let expected = expected_contract
        .origins()
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    actual_contract.origins().iter().all(|actual| {
        input_correspondence
            .iter()
            .any(|(interface, implementation)| {
                implementation == actual && expected.contains(interface)
            })
    })
}

fn input_correspondence(
    interface: &CallableDeclaration,
    selected: &CallableDeclaration,
) -> Option<InterfaceImplementationInputCorrespondence> {
    if interface.parameters().len() != selected.parameters().len()
        || interface.receiver().is_some() != selected.receiver().is_some()
    {
        return None;
    }
    let receiver = interface
        .receiver()
        .is_some()
        .then_some((ProvenanceOrigin::Receiver, ProvenanceOrigin::Receiver));
    let parameters = interface
        .parameters()
        .iter()
        .copied()
        .zip(selected.parameters().iter().copied())
        .map(|(interface, selected)| {
            (
                ProvenanceOrigin::Parameter(interface),
                ProvenanceOrigin::Parameter(selected),
            )
        });
    Some(receiver.into_iter().chain(parameters).collect())
}

fn receiver_capability(
    graph: &DeclarationGraph,
    receiver: Option<ParameterId>,
) -> Result<Option<nocter_model::CallableCapability>, InterfaceImplementationInternalError> {
    receiver
        .map(|receiver| {
            let parameter = graph.declarations().parameters().get(receiver).ok_or(
                InterfaceImplementationInternalError::MissingParameter(receiver),
            )?;
            match parameter.role() {
                nocter_declarations::ParameterRole::Receiver(capability) => Ok(capability),
                nocter_declarations::ParameterRole::Ordinary { .. }
                | nocter_declarations::ParameterRole::ArgumentPack { .. } => Err(
                    InterfaceImplementationInternalError::MissingParameter(receiver),
                ),
            }
        })
        .transpose()
}

fn parameter_contract(
    graph: &DeclarationGraph,
    parameter: ParameterId,
) -> Result<(nocter_model::TypeId, bool), InterfaceImplementationInternalError> {
    graph
        .declarations()
        .parameters()
        .get(parameter)
        .and_then(|parameter| match parameter.role() {
            nocter_declarations::ParameterRole::Ordinary { .. } => Some((parameter.ty(), false)),
            nocter_declarations::ParameterRole::ArgumentPack { .. } => Some((parameter.ty(), true)),
            nocter_declarations::ParameterRole::Receiver(_) => None,
        })
        .ok_or(InterfaceImplementationInternalError::MissingParameter(
            parameter,
        ))
}

fn site_origin(
    source_index: DiagnosticOrigins<'_>,
    site: nocter_model::DeclarationSiteId,
) -> Result<SourceOrigin, InterfaceImplementationInternalError> {
    let entity = SemanticEntity::DeclarationSite(site);
    source_index
        .declaration(entity)
        .ok_or(InterfaceImplementationInternalError::MissingSource(entity))
}

fn interface_id_index(
    interface: InterfaceId,
    interfaces: &nocter_model::Arena<InterfaceId, nocter_declarations::InterfaceDeclaration>,
) -> Result<usize, InterfaceImplementationInternalError> {
    interfaces
        .iter()
        .position(|(candidate, _)| candidate == interface)
        .ok_or(InterfaceImplementationInternalError::MissingInterface(
            interface,
        ))
}
