use std::collections::{BTreeMap, HashSet};
use std::fmt;

use nocter_declarations::{
    AssociatedTypeBinding, CallableDeclaration, CallableProvenanceContract, DeclarationGraph,
    InterfaceApplication, ProvenanceOrigin,
};
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{
    ArenaBuilder, CallableId, ConformanceId, GenericParameterId, InterfaceId, ParameterId,
    TypeKind, TypeStore,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin};

use super::diagnostic;
use super::model::{CheckedConformance, ConformanceMethod, ConformanceTable, MethodSelection};
use super::overlap::patterns_overlap;
use super::predicate::{CheckedRequirement, normalize_requirements};
use super::required_method::RequiredConformanceMethod;
use super::validate::validate_associated_bounds;
use crate::declaration_patterns::DeclarationPatternTable;
use crate::type_relations::{SubstitutionError, TypeSubstitution};

/// Authored conformance failure or an inconsistent semantic boundary.
#[derive(Debug)]
pub enum ConformanceBuildError {
    Rule {
        diagnostic: Box<SourceDiagnostic>,
        missing_methods: Option<Box<MissingConformanceMethods>>,
    },
    Internal(ConformanceInternalError),
}

impl ConformanceBuildError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule { diagnostic, .. } => Some(diagnostic),
            Self::Internal(_) => None,
        }
    }

    #[must_use]
    pub fn missing_methods(&self) -> Option<&MissingConformanceMethods> {
        match self {
            Self::Rule {
                missing_methods, ..
            } => missing_methods.as_deref(),
            Self::Internal(_) => None,
        }
    }
}

impl fmt::Display for ConformanceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule { diagnostic, .. } => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ConformanceBuildError {}

impl From<SourceDiagnostic> for ConformanceBuildError {
    fn from(diagnostic: SourceDiagnostic) -> Self {
        Self::Rule {
            diagnostic: Box::new(diagnostic),
            missing_methods: None,
        }
    }
}

/// Exact specialized signatures missing from one conformance declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingConformanceMethods {
    conformance: ConformanceId,
    required: Box<[RequiredConformanceMethod]>,
}

impl MissingConformanceMethods {
    #[must_use]
    pub(super) fn new(
        conformance: ConformanceId,
        required: impl Into<Box<[RequiredConformanceMethod]>>,
    ) -> Self {
        Self {
            conformance,
            required: required.into(),
        }
    }

    #[must_use]
    pub const fn conformance(&self) -> ConformanceId {
        self.conformance
    }

    #[must_use]
    pub const fn required(&self) -> &[RequiredConformanceMethod] {
        &self.required
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

impl From<SubstitutionError> for ConformanceInternalError {
    fn from(error: SubstitutionError) -> Self {
        Self::Substitution(error)
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
#[cfg(test)]
pub(super) fn build_conformance_table(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source_index: &SourceIndex,
) -> Result<ConformanceTable, ConformanceBuildError> {
    let patterns = DeclarationPatternTable::build(graph, types)?;
    let operations = crate::admitted_operations::AdmittedOperations::new(graph, None);
    build_conformance_table_from_ids(
        graph,
        types,
        source_index,
        &patterns,
        operations.conformances(),
    )
}

pub(crate) fn build_conformance_table_from_ids(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source_index: &SourceIndex,
    patterns: &DeclarationPatternTable,
    conformances: &[ConformanceId],
) -> Result<ConformanceTable, ConformanceBuildError> {
    let declarations = graph.declarations();
    let mut entries = BTreeMap::new();
    let mut by_interface = vec![Vec::new(); declarations.interfaces().len()];
    let mut preceding_patterns = BTreeMap::<InterfaceId, Vec<_>>::new();

    for id in conformances {
        let id = *id;
        let conformance = declarations
            .conformances()
            .get(id)
            .ok_or(ConformanceInternalError::MissingConformance(id))?;
        let pattern = patterns
            .conformance(id)
            .ok_or(ConformanceInternalError::MissingConformance(id))?;
        let normalized_interface = pattern.interface().clone();
        let normalized_target = pattern.target();
        let interface_id = normalized_interface.interface();
        let interface = declarations
            .interfaces()
            .get(interface_id)
            .ok_or(ConformanceInternalError::MissingInterface(interface_id))?;
        record_nonoverlapping_pattern(
            types,
            source_index,
            &mut preceding_patterns,
            ConformancePattern {
                site: conformance.site(),
                interface: normalized_interface.clone(),
                target: normalized_target,
            },
        )?;

        let associated_types = pattern.associated_types();
        let substitution = conformance_substitution(
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
            implementation_methods: conformance.methods(),
            expected_substitution: &substitution,
            actual_substitution: pattern.substitution(),
            conformance_site: conformance.site(),
            conformance: id,
        })?;
        let previous = entries.insert(
            id,
            CheckedConformance::new(
                normalized_interface,
                normalized_target,
                conformance.generic_parameters(),
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
    let table = ConformanceTable::new(
        entries,
        by_interface_arena.finish(),
        method_interface_index(graph)?,
    );
    validate_associated_bounds(graph, types, source_index, &table)?;
    Ok(table)
}

fn method_interface_index(
    graph: &DeclarationGraph,
) -> Result<BTreeMap<nocter_model::Symbol, Box<[InterfaceId]>>, ConformanceInternalError> {
    let declarations = graph.declarations();
    let mut by_method = BTreeMap::<_, Vec<_>>::new();
    for (interface_id, interface) in declarations.interfaces().iter() {
        for method in interface.methods() {
            let callable = declarations
                .callables()
                .get(*method)
                .ok_or(ConformanceInternalError::MissingCallable(*method))?;
            let name = callable
                .name()
                .ok_or(ConformanceInternalError::MissingCallable(*method))?;
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

struct ConformancePattern {
    site: nocter_model::DeclarationSiteId,
    interface: InterfaceApplication,
    target: nocter_model::TypeId,
}

type ConformancePatterns = BTreeMap<InterfaceId, Vec<ConformancePattern>>;

fn record_nonoverlapping_pattern(
    types: &mut TypeStore,
    source_index: &SourceIndex,
    preceding_patterns: &mut ConformancePatterns,
    current: ConformancePattern,
) -> Result<(), ConformanceBuildError> {
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
    types: &'program mut TypeStore,
    source_index: &'program SourceIndex,
    interface_id: InterfaceId,
    interface_methods: &'program [CallableId],
    implementation_methods: &'program [CallableId],
    expected_substitution: &'program TypeSubstitution,
    actual_substitution: &'program TypeSubstitution,
    conformance_site: nocter_model::DeclarationSiteId,
    conformance: ConformanceId,
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
        conformance,
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
                    site_origin(source_index, actual.site())?,
                    site_origin(source_index, expected.site())?,
                )
                .into());
            }
            MethodSelection::Implementation(implementation)
        } else if expected.body().is_some() {
            MethodSelection::Default(*interface_method)
        } else {
            missing.push(RequiredConformanceMethod::build(
                graph,
                types,
                conformance,
                *interface_method,
                expected,
                expected_substitution,
            )?);
            continue;
        };
        selected.push(ConformanceMethod::new(*interface_method, selection));
    }
    if !missing.is_empty() {
        return Err(missing_methods_error(
            graph,
            source_index,
            conformance_site,
            conformance,
            missing,
        )?);
    }
    selected.sort_unstable_by_key(|method| method.interface_method());
    Ok(selected)
}

fn interface_method_index(
    graph: &DeclarationGraph,
    interface: InterfaceId,
    methods: &[CallableId],
) -> Result<BTreeMap<nocter_model::Symbol, CallableId>, ConformanceInternalError> {
    let mut by_name = BTreeMap::new();
    for method in methods {
        let callable = graph
            .declarations()
            .callables()
            .get(*method)
            .ok_or(ConformanceInternalError::MissingCallable(*method))?;
        let name = callable
            .name()
            .ok_or(ConformanceInternalError::MissingCallable(*method))?;
        if by_name.insert(name, *method).is_some() {
            return Err(ConformanceInternalError::InvalidInterfaceMethodSet(
                interface,
            ));
        }
    }
    Ok(by_name)
}

fn implementation_method_index(
    graph: &DeclarationGraph,
    source_index: &SourceIndex,
    interface: InterfaceId,
    methods: &[CallableId],
    expected: &BTreeMap<nocter_model::Symbol, CallableId>,
) -> Result<BTreeMap<nocter_model::Symbol, CallableId>, ConformanceBuildError> {
    let declarations = graph.declarations();
    let interface_site = declarations
        .interfaces()
        .get(interface)
        .ok_or(ConformanceInternalError::MissingInterface(interface))?
        .site();
    let mut by_name = BTreeMap::new();
    for method in methods {
        let callable = declarations
            .callables()
            .get(*method)
            .ok_or(ConformanceInternalError::MissingCallable(*method))?;
        let name = callable
            .name()
            .ok_or(ConformanceInternalError::MissingCallable(*method))?;
        if by_name.insert(name, *method).is_some() || !expected.contains_key(&name) {
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
    source_index: &SourceIndex,
    conformance_site: nocter_model::DeclarationSiteId,
    conformance: ConformanceId,
    missing: Vec<RequiredConformanceMethod>,
) -> Result<ConformanceBuildError, ConformanceInternalError> {
    let first = missing
        .first()
        .expect("missing conformance repair is never empty");
    let expected = graph
        .declarations()
        .callables()
        .get(first.interface_method())
        .ok_or(ConformanceInternalError::MissingCallable(
            first.interface_method(),
        ))?;
    Ok(ConformanceBuildError::Rule {
        diagnostic: Box::new(diagnostic::missing_method(
            site_origin(source_index, conformance_site)?,
            site_origin(source_index, expected.site())?,
        )),
        missing_methods: Some(Box::new(MissingConformanceMethods::new(
            conformance,
            missing,
        ))),
    })
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
        let (expected_type, expected_pack) = parameter_contract(graph, *expected)?;
        let (actual_type, actual_pack) = parameter_contract(graph, *actual)?;
        if expected_pack != actual_pack
            || substitution.apply_type(types, expected_type)?
                != actual_substitution.apply_type(types, actual_type)?
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
                nocter_declarations::ParameterRole::Ordinary { .. }
                | nocter_declarations::ParameterRole::ArgumentPack { .. } => {
                    Err(ConformanceInternalError::MissingParameter(receiver))
                }
            }
        })
        .transpose()
}

fn parameter_contract(
    graph: &DeclarationGraph,
    parameter: ParameterId,
) -> Result<(nocter_model::TypeId, bool), ConformanceInternalError> {
    graph
        .declarations()
        .parameters()
        .get(parameter)
        .and_then(|parameter| match parameter.role() {
            nocter_declarations::ParameterRole::Ordinary { .. } => Some((parameter.ty(), false)),
            nocter_declarations::ParameterRole::ArgumentPack { .. } => Some((parameter.ty(), true)),
            nocter_declarations::ParameterRole::Receiver(_) => None,
        })
        .ok_or(ConformanceInternalError::MissingParameter(parameter))
}

fn site_origin(
    source_index: &SourceIndex,
    site: nocter_model::DeclarationSiteId,
) -> Result<SourceOrigin, ConformanceInternalError> {
    let entity = SemanticEntity::DeclarationSite(site);
    crate::diagnostic_projection::declaration_origin(source_index, entity)
        .ok_or(ConformanceInternalError::MissingSource(entity))
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
