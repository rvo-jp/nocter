use std::collections::{BTreeMap, HashSet};
use std::fmt;

use nocter_declarations::{
    AssociatedTypeBinding, CallableDeclaration, CallableProvenanceContract, ConformanceDeclaration,
    DeclarationGraph, InterfaceApplication, ProvenanceOrigin, RequirementKind,
};
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{
    ArenaBuilder, CallableId, GenericParameterId, InterfaceId, ParameterId, TypeKind, TypeStore,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};

use super::diagnostic;
use super::model::{CheckedConformance, ConformanceMethod, ConformanceTable, MethodSelection};
use super::overlap::patterns_overlap;
use super::predicate::{CheckedRequirement, normalize_requirements};
use super::substitution::{SubstitutionError, TypeSubstitution};
use super::validate::validate_associated_bounds;

/// Authored conformance failure or an inconsistent semantic boundary.
#[derive(Debug)]
pub enum ConformanceBuildError {
    Rule(SourceDiagnostic),
    Internal(ConformanceInternalError),
}

impl ConformanceBuildError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule(diagnostic) => Some(diagnostic),
            Self::Internal(_) => None,
        }
    }
}

impl fmt::Display for ConformanceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(diagnostic) => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConformanceBuildError {}

impl From<SourceDiagnostic> for ConformanceBuildError {
    fn from(diagnostic: SourceDiagnostic) -> Self {
        Self::Rule(diagnostic)
    }
}

impl From<ConformanceInternalError> for ConformanceBuildError {
    fn from(error: ConformanceInternalError) -> Self {
        Self::Internal(error)
    }
}

impl From<SubstitutionError> for ConformanceBuildError {
    fn from(error: SubstitutionError) -> Self {
        Self::Internal(ConformanceInternalError::Substitution(error))
    }
}

#[derive(Debug)]
pub enum ConformanceInternalError {
    MissingInterface(InterfaceId),
    MissingConformance(nocter_model::ConformanceId),
    MissingAssociatedType(nocter_model::AssociatedTypeId),
    MissingCallable(CallableId),
    MissingParameter(ParameterId),
    MissingSource(SemanticEntity),
    InvalidGenericType(GenericParameterId),
    InvalidInterfaceMethodSet(InterfaceId),
    Substitution(SubstitutionError),
}

impl fmt::Display for ConformanceInternalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInterface(interface) => {
                write!(formatter, "missing conformance interface {interface:?}")
            }
            Self::MissingConformance(conformance) => {
                write!(formatter, "missing conformance {conformance:?}")
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

impl std::error::Error for ConformanceInternalError {}

/// Builds the sole program-wide conformance dispatch table.
///
/// The declaration graph and type store must come from one consumed `DeclarationProgram`.
/// Signature comparison interns specialized expected types into that same store.
///
/// # Errors
///
/// Returns a source-backed error for missing, extra, incompatible, or overlapping conformance
/// declarations, and an internal error for an inconsistent declaration/source graph.
pub fn build_conformance_table(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source_index: &SourceIndex,
) -> Result<ConformanceTable, ConformanceBuildError> {
    let declarations = graph.declarations();
    let mut entries = ArenaBuilder::new();
    let mut by_interface = vec![Vec::new(); declarations.interfaces().len()];
    let mut preceding_patterns = BTreeMap::<InterfaceId, Vec<_>>::new();

    for (id, conformance) in declarations.conformances().iter() {
        let pattern_substitution = conformance_pattern_substitution(graph, conformance)?;
        let normalized_interface = InterfaceApplication::new(
            conformance.interface().interface(),
            conformance
                .interface()
                .arguments()
                .iter()
                .map(|argument| pattern_substitution.apply_type(types, *argument))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let normalized_target = pattern_substitution.apply_type(types, conformance.target())?;
        let interface_id = normalized_interface.interface();
        let interface = declarations
            .interfaces()
            .get(interface_id)
            .ok_or(ConformanceInternalError::MissingInterface(interface_id))?;
        if let Some(preceding) = preceding_patterns.get(&interface_id) {
            for (previous, previous_interface, previous_target) in preceding {
                if patterns_overlap(
                    types,
                    previous_interface,
                    *previous_target,
                    &normalized_interface,
                    normalized_target,
                )? {
                    return Err(diagnostic::overlapping(
                        site_origin(graph, source_index, conformance.site())?,
                        site_origin(
                            graph,
                            source_index,
                            declarations
                                .conformances()
                                .get(*previous)
                                .ok_or(ConformanceInternalError::MissingConformance(*previous))?
                                .site(),
                        )?,
                    )
                    .into());
                }
            }
        }
        preceding_patterns.entry(interface_id).or_default().push((
            id,
            normalized_interface.clone(),
            normalized_target,
        ));

        let mut associated_types = conformance
            .associated_types()
            .iter()
            .map(|binding| {
                pattern_substitution
                    .apply_type(types, binding.ty())
                    .map(|ty| AssociatedTypeBinding::new(binding.declaration(), ty))
            })
            .collect::<Result<Vec<_>, _>>()?;
        associated_types.sort_unstable_by_key(|binding| binding.declaration());
        let substitution = conformance_substitution(
            graph,
            &normalized_interface,
            normalized_target,
            &associated_types,
        )?;
        let methods = select_methods(MethodSelectionInput {
            graph,
            types,
            source_index,
            interface_id,
            interface_methods: interface.methods(),
            implementation_methods: conformance.methods(),
            expected_substitution: &substitution,
            actual_substitution: &pattern_substitution,
            conformance_site: conformance.site(),
        })?;
        let requirements =
            normalize_conformance_requirements(graph, types, &pattern_substitution, conformance)?;
        let actual = entries.insert(CheckedConformance::new(
            normalized_interface,
            normalized_target,
            requirements,
            associated_types,
            methods,
        ));
        debug_assert_eq!(actual, id);
        by_interface[interface_id_index(interface_id, declarations.interfaces())?].push(id);
    }

    let mut by_interface_arena = ArenaBuilder::new();
    for candidates in by_interface {
        by_interface_arena.insert(candidates.into_boxed_slice());
    }
    let table = ConformanceTable::new(entries.finish(), by_interface_arena.finish());
    validate_associated_bounds(graph, types, source_index, &table)?;
    Ok(table)
}

struct MethodSelectionInput<'program> {
    graph: &'program DeclarationGraph,
    types: &'program mut TypeStore,
    source_index: &'program SourceIndex,
    interface_id: InterfaceId,
    interface_methods: &'program [CallableId],
    implementation_methods: &'program [CallableId],
    expected_substitution: &'program TypeSubstitution,
    actual_substitution: &'program TypeSubstitution,
    conformance_site: nocter_model::DeclarationSiteId,
}

fn select_methods(
    input: MethodSelectionInput<'_>,
) -> Result<Vec<ConformanceMethod>, ConformanceBuildError> {
    let MethodSelectionInput {
        graph,
        types,
        source_index,
        interface_id,
        interface_methods,
        implementation_methods,
        expected_substitution,
        actual_substitution,
        conformance_site,
    } = input;
    let declarations = graph.declarations();
    let mut interface_by_name = BTreeMap::new();
    for method in interface_methods {
        let callable = declarations
            .callables()
            .get(*method)
            .ok_or(ConformanceInternalError::MissingCallable(*method))?;
        let name = callable
            .name()
            .ok_or(ConformanceInternalError::MissingCallable(*method))?;
        if interface_by_name.insert(name, *method).is_some() {
            return Err(ConformanceInternalError::InvalidInterfaceMethodSet(interface_id).into());
        }
    }

    let mut implementation_by_name = BTreeMap::new();
    for method in implementation_methods {
        let callable = declarations
            .callables()
            .get(*method)
            .ok_or(ConformanceInternalError::MissingCallable(*method))?;
        let name = callable
            .name()
            .ok_or(ConformanceInternalError::MissingCallable(*method))?;
        if implementation_by_name.insert(name, *method).is_some()
            || !interface_by_name.contains_key(&name)
        {
            return Err(diagnostic::extra_method(
                site_origin(graph, source_index, callable.site())?,
                site_origin(
                    graph,
                    source_index,
                    declarations
                        .interfaces()
                        .get(interface_id)
                        .ok_or(ConformanceInternalError::MissingInterface(interface_id))?
                        .site(),
                )?,
            )
            .into());
        }
    }

    let mut selected = Vec::with_capacity(interface_methods.len());
    for interface_method in interface_methods {
        let expected = declarations
            .callables()
            .get(*interface_method)
            .ok_or(ConformanceInternalError::MissingCallable(*interface_method))?;
        let name = expected
            .name()
            .ok_or(ConformanceInternalError::MissingCallable(*interface_method))?;
        let selection = if let Some(implementation) = implementation_by_name.get(&name).copied() {
            let actual = declarations
                .callables()
                .get(implementation)
                .ok_or(ConformanceInternalError::MissingCallable(implementation))?;
            if !compatible_signature(
                graph,
                types,
                expected,
                actual,
                expected_substitution,
                actual_substitution,
            )? {
                return Err(diagnostic::incompatible_method(
                    site_origin(graph, source_index, actual.site())?,
                    site_origin(graph, source_index, expected.site())?,
                )
                .into());
            }
            MethodSelection::Implementation(implementation)
        } else if expected.body().is_some() {
            MethodSelection::Default(*interface_method)
        } else {
            return Err(diagnostic::missing_method(
                site_origin(graph, source_index, conformance_site)?,
                site_origin(graph, source_index, expected.site())?,
            )
            .into());
        };
        selected.push(ConformanceMethod::new(*interface_method, selection));
    }
    selected.sort_unstable_by_key(|method| method.interface_method());
    Ok(selected)
}

fn compatible_signature(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    expected: &CallableDeclaration,
    actual: &CallableDeclaration,
    owner_substitution: &TypeSubstitution,
    actual_substitution: &TypeSubstitution,
) -> Result<bool, ConformanceBuildError> {
    if expected.kind() != actual.kind()
        || expected.parameters().len() != actual.parameters().len()
        || expected.generic_parameters().len() != actual.generic_parameters().len()
    {
        return Ok(false);
    }
    let mut substitution = owner_substitution.clone();
    for (expected, actual) in expected
        .generic_parameters()
        .iter()
        .zip(actual.generic_parameters())
    {
        let ty = types
            .intern(TypeKind::GenericParameter(*actual))
            .map_err(|_| ConformanceInternalError::InvalidGenericType(*actual))?;
        substitution.bind_generic(*expected, ty);
    }
    if receiver_capability(graph, expected.receiver())?
        != receiver_capability(graph, actual.receiver())?
    {
        return Ok(false);
    }
    for (expected, actual) in expected.parameters().iter().zip(actual.parameters()) {
        let expected = parameter_type(graph, *expected)?;
        let actual = parameter_type(graph, *actual)?;
        if substitution.apply_type(types, expected)?
            != actual_substitution.apply_type(types, actual)?
        {
            return Ok(false);
        }
    }
    if substitution.apply_type(types, expected.result())?
        != actual_substitution.apply_type(types, actual.result())?
    {
        return Ok(false);
    }
    let expected_requirements =
        normalize_requirements(graph, types, &substitution, expected.requirements())?;
    let actual_requirements =
        normalize_requirements(graph, types, actual_substitution, actual.requirements())?;
    if !same_predicates(&expected_requirements, &actual_requirements) {
        return Ok(false);
    }
    compatible_provenance(expected, actual)
}

fn conformance_substitution(
    graph: &DeclarationGraph,
    normalized_interface: &InterfaceApplication,
    normalized_target: nocter_model::TypeId,
    associated_types: &[AssociatedTypeBinding],
) -> Result<TypeSubstitution, ConformanceBuildError> {
    let interface_id = normalized_interface.interface();
    let interface = graph
        .declarations()
        .interfaces()
        .get(interface_id)
        .ok_or(ConformanceInternalError::MissingInterface(interface_id))?;
    let mut substitution = TypeSubstitution::default();
    substitution.set_interface_self(interface_id, normalized_target);
    for (parameter, argument) in interface
        .generic_parameters()
        .iter()
        .zip(normalized_interface.arguments())
    {
        substitution.bind_type(*parameter, *argument);
    }
    for binding in associated_types {
        substitution.bind_associated(binding.declaration(), binding.ty());
    }
    Ok(substitution)
}

fn conformance_pattern_substitution(
    graph: &DeclarationGraph,
    conformance: &ConformanceDeclaration,
) -> Result<TypeSubstitution, ConformanceBuildError> {
    let mut substitution = TypeSubstitution::default();
    for requirement in conformance.requirements() {
        let requirement = graph
            .declarations()
            .requirements()
            .get(*requirement)
            .ok_or(SubstitutionError::InvalidStore)?;
        if let RequirementKind::BinderRefinement {
            parameter,
            replacement,
        } = requirement.kind()
        {
            substitution.bind_generic(*parameter, *replacement);
        }
    }
    Ok(substitution)
}

fn normalize_conformance_requirements(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    substitution: &TypeSubstitution,
    conformance: &ConformanceDeclaration,
) -> Result<Vec<CheckedRequirement>, ConformanceBuildError> {
    let retained = conformance
        .requirements()
        .iter()
        .copied()
        .filter(|requirement| {
            !matches!(
                graph
                    .declarations()
                    .requirements()
                    .get(*requirement)
                    .map(nocter_declarations::Requirement::kind),
                Some(RequirementKind::BinderRefinement { .. })
            )
        })
        .collect::<Vec<_>>();
    Ok(normalize_requirements(
        graph,
        types,
        substitution,
        &retained,
    )?)
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
) -> Result<bool, ConformanceBuildError> {
    let (
        CallableProvenanceContract::Declared(expected_contract),
        CallableProvenanceContract::Declared(actual_contract),
    ) = (
        expected_declaration.provenance(),
        actual_declaration.provenance(),
    )
    else {
        return Ok(true);
    };
    let expected = provenance_positions(expected_contract.origins(), expected_declaration)?;
    let actual = provenance_positions(actual_contract.origins(), actual_declaration)?;
    Ok(actual.is_subset(&expected))
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ProvenancePosition {
    Receiver,
    Parameter(usize),
}

fn provenance_positions(
    origins: &[ProvenanceOrigin],
    callable: &CallableDeclaration,
) -> Result<HashSet<ProvenancePosition>, ConformanceBuildError> {
    origins
        .iter()
        .map(|origin| match origin {
            ProvenanceOrigin::Receiver => Ok(ProvenancePosition::Receiver),
            ProvenanceOrigin::Parameter(parameter) => callable
                .parameters()
                .iter()
                .position(|candidate| candidate == parameter)
                .map(ProvenancePosition::Parameter)
                .ok_or_else(|| ConformanceInternalError::MissingParameter(*parameter).into()),
        })
        .collect()
}

fn receiver_capability(
    graph: &DeclarationGraph,
    receiver: Option<ParameterId>,
) -> Result<Option<nocter_model::CallableCapability>, ConformanceInternalError> {
    receiver
        .map(|receiver| {
            let parameter = graph
                .declarations()
                .parameters()
                .get(receiver)
                .ok_or(ConformanceInternalError::MissingParameter(receiver))?;
            match parameter.role() {
                nocter_declarations::ParameterRole::Receiver(capability) => Ok(capability),
                nocter_declarations::ParameterRole::Ordinary { .. } => {
                    Err(ConformanceInternalError::MissingParameter(receiver))
                }
            }
        })
        .transpose()
}

fn parameter_type(
    graph: &DeclarationGraph,
    parameter: ParameterId,
) -> Result<nocter_model::TypeId, ConformanceInternalError> {
    graph
        .declarations()
        .parameters()
        .get(parameter)
        .map(|parameter| parameter.ty())
        .ok_or(ConformanceInternalError::MissingParameter(parameter))
}

fn site_origin(
    _graph: &DeclarationGraph,
    source_index: &SourceIndex,
    site: nocter_model::DeclarationSiteId,
) -> Result<SourceOrigin, ConformanceInternalError> {
    source_index
        .bindings_for(SemanticEntity::DeclarationSite(site))
        .iter()
        .find(|binding| {
            matches!(
                binding.role(),
                SourceRole::Declaration | SourceRole::Implementation
            )
        })
        .map(|binding| binding.origin())
        .ok_or(ConformanceInternalError::MissingSource(
            SemanticEntity::DeclarationSite(site),
        ))
}

fn interface_id_index(
    interface: InterfaceId,
    interfaces: &nocter_model::Arena<InterfaceId, nocter_declarations::InterfaceDeclaration>,
) -> Result<usize, ConformanceInternalError> {
    interfaces
        .iter()
        .position(|(candidate, _)| candidate == interface)
        .ok_or(ConformanceInternalError::MissingInterface(interface))
}
