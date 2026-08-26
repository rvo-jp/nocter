use std::collections::{HashMap, HashSet};
use std::fmt;

use nocter_declarations::{
    AssociatedTypeBinding, InterfaceApplication, RequirementKind, RequirementSubject,
};
use nocter_frontend_bindings::AssociatedProjectionUse;
use nocter_model::{
    AssociatedTypeId, CallableContract, GenericParameterId, InterfaceId, OpaqueTypeId,
    ParameterOrigin, ResultProvenance, Symbol, TypeAliasId, TypeId, TypeKind, TypeStore,
};
use nocter_syntax::NodeId;

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclaration, SurfaceDeclarationId};

use super::normalization_origins::NormalizationOrigins;
use super::{
    BoundInterfaceApplication, BoundOpaqueResult, BoundRequirementKind, BoundTypeId, BoundTypeKind,
    PreparedTypeBindings, TypeBindingRule, TypeBindingViolation,
};

mod preparation;
mod violation;

use preparation::prepare_context;
pub use violation::{TypeNormalizationRule, TypeNormalizationViolation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NormalizedDeclarationPattern {
    Type(TypeId),
    Interface(InterfaceApplication),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedOpaqueResult {
    generic_parameters: Box<[GenericParameterId]>,
    interface: InterfaceApplication,
    associated_types: Box<[AssociatedTypeBinding]>,
    result: TypeId,
}

impl NormalizedOpaqueResult {
    #[must_use]
    pub const fn generic_parameters(&self) -> &[GenericParameterId] {
        &self.generic_parameters
    }

    #[must_use]
    pub const fn interface(&self) -> &InterfaceApplication {
        &self.interface
    }

    #[must_use]
    pub const fn associated_types(&self) -> &[AssociatedTypeBinding] {
        &self.associated_types
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeNormalizationError {
    Rule(TypeNormalizationViolation),
    RequirementRule(TypeBindingViolation),
    InvalidBoundType(BoundTypeId),
    InconsistentTypeStore,
    MissingInterfaceApplicationContext(NodeId),
    MissingAlias(TypeAliasId),
    InvalidSelf(ReservedEntity),
    InconsistentAssociatedIndex(SurfaceDeclarationId),
}

impl fmt::Display for TypeNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(violation) => write!(
                formatter,
                "{}: {}",
                violation.rule().code(),
                violation.rule().message()
            ),
            Self::RequirementRule(violation) => write!(
                formatter,
                "{}: {}",
                violation.rule().code(),
                violation.rule().message()
            ),
            Self::InvalidBoundType(ty) => write!(formatter, "bound type {ty:?} is inconsistent"),
            Self::InconsistentTypeStore => {
                formatter.write_str("normalized type store contains an invalid reference")
            }
            Self::MissingInterfaceApplicationContext(node) => {
                write!(
                    formatter,
                    "interface application {node:?} has no declaration context"
                )
            }
            Self::MissingAlias(alias) => write!(formatter, "type alias {alias:?} has no target"),
            Self::InvalidSelf(owner) => write!(formatter, "{owner:?} has no normalized Self type"),
            Self::InconsistentAssociatedIndex(declaration) => write!(
                formatter,
                "declaration {declaration:?} duplicates an associated index entry"
            ),
        }
    }
}

impl std::error::Error for TypeNormalizationError {}

impl From<TypeNormalizationViolation> for TypeNormalizationError {
    fn from(violation: TypeNormalizationViolation) -> Self {
        Self::Rule(violation)
    }
}

/// Header types after aliases, `Self`, associated names, and requirement types are canonicalized.
#[derive(Debug)]
pub struct PreparedTypes<'syntax> {
    pub(crate) namespaces: PreparedNamespaces<'syntax>,
    pub(crate) roots: HashMap<NodeId, TypeId>,
    pub(crate) alias_targets: HashMap<TypeAliasId, TypeId>,
    pub(crate) patterns: Box<[Box<[NormalizedDeclarationPattern]>]>,
    pub(crate) interface_applications: HashMap<NodeId, InterfaceApplication>,
    pub(crate) opaque_results: HashMap<OpaqueTypeId, NormalizedOpaqueResult>,
    pub(crate) callable_results: Box<[Option<TypeId>]>,
    pub(crate) requirements: Box<[Box<[RequirementKind]>]>,
    pub(crate) constant_values: HashMap<nocter_model::ConstantId, super::PreparedConstantValue>,
    pub(crate) associated_projection_uses: Box<[AssociatedProjectionUse]>,
}

impl PreparedTypes<'_> {
    #[must_use]
    pub const fn namespaces(&self) -> &PreparedNamespaces<'_> {
        &self.namespaces
    }

    #[must_use]
    pub fn type_for(&self, node: NodeId) -> Option<TypeId> {
        self.roots.get(&node).copied()
    }

    #[must_use]
    pub fn alias_target(&self, alias: TypeAliasId) -> Option<TypeId> {
        self.alias_targets.get(&alias).copied()
    }

    #[must_use]
    pub fn declaration_patterns(
        &self,
        declaration: SurfaceDeclarationId,
    ) -> Option<&[NormalizedDeclarationPattern]> {
        self.patterns.get(declaration.index()).map(AsRef::as_ref)
    }

    #[must_use]
    pub fn interface_application_for(&self, node: NodeId) -> Option<&InterfaceApplication> {
        self.interface_applications.get(&node)
    }

    #[must_use]
    pub fn opaque_result(&self, opaque: OpaqueTypeId) -> Option<&NormalizedOpaqueResult> {
        self.opaque_results.get(&opaque)
    }

    #[must_use]
    pub fn callable_result(&self, declaration: SurfaceDeclarationId) -> Option<TypeId> {
        self.callable_results
            .get(declaration.index())
            .copied()
            .flatten()
    }

    #[must_use]
    pub fn declaration_requirements(
        &self,
        declaration: SurfaceDeclarationId,
    ) -> Option<&[RequirementKind]> {
        self.requirements
            .get(declaration.index())
            .map(AsRef::as_ref)
    }
}

#[derive(Clone, Debug)]
struct AliasDefinition {
    declaration: SurfaceDeclarationId,
    parameters: Box<[GenericParameterId]>,
    target: BoundTypeId,
}

#[derive(Debug)]
struct NormalizationContext {
    declarations: Box<[SurfaceDeclaration]>,
    entity_declarations: std::collections::BTreeMap<ReservedEntity, SurfaceDeclarationId>,
    aliases: HashMap<TypeAliasId, AliasDefinition>,
    associated: HashMap<(InterfaceId, Symbol), AssociatedTypeId>,
    associated_surfaces: HashMap<AssociatedTypeId, SurfaceDeclarationId>,
    self_types: HashMap<ReservedEntity, TypeId>,
    patterns: Box<[Box<[NormalizedDeclarationPattern]>]>,
    implementation_interfaces: HashMap<SurfaceDeclarationId, InterfaceId>,
    bound_requirements: Box<[Box<[BoundRequirementKind]>]>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct EvaluationKey {
    ty: BoundTypeId,
    declaration: SurfaceDeclarationId,
    substitutions: Box<[(GenericParameterId, TypeId)]>,
}

impl EvaluationKey {
    fn child(&self, ty: BoundTypeId) -> Self {
        Self {
            ty,
            declaration: self.declaration,
            substitutions: self.substitutions.clone(),
        }
    }
}

enum EvaluationFrame {
    Enter(EvaluationKey),
    Finish {
        key: EvaluationKey,
        kind: BoundTypeKind,
    },
    AliasArguments {
        key: EvaluationKey,
        definition: TypeAliasId,
        arguments: Vec<EvaluationKey>,
    },
    AliasTarget {
        key: EvaluationKey,
        definition: TypeAliasId,
        target: EvaluationKey,
    },
}

struct Evaluator<'a> {
    kinds: &'a [BoundTypeKind],
    origins: &'a NormalizationOrigins,
    context: &'a NormalizationContext,
    store: &'a mut nocter_model::TypeTransaction,
    memo: HashMap<EvaluationKey, TypeId>,
    active: HashSet<EvaluationKey>,
    alias_stack: Vec<TypeAliasId>,
    array_lengths: &'a HashMap<NodeId, u64>,
    associated_projection_uses: Vec<AssociatedProjectionUse>,
}

impl Evaluator<'_> {
    fn normalize(
        &mut self,
        ty: BoundTypeId,
        declaration: SurfaceDeclarationId,
    ) -> Result<TypeId, TypeNormalizationError> {
        let root = EvaluationKey {
            ty,
            declaration,
            substitutions: Box::new([]),
        };
        if let Some(normalized) = self.memo.get(&root) {
            return Ok(*normalized);
        }
        let mut frames = vec![EvaluationFrame::Enter(root.clone())];
        while let Some(frame) = frames.pop() {
            match frame {
                EvaluationFrame::Enter(key) => self.enter(key, &mut frames)?,
                EvaluationFrame::Finish { key, kind } => self.finish(key, kind)?,
                EvaluationFrame::AliasArguments {
                    key,
                    definition,
                    arguments,
                } => self.expand_alias(key, definition, &arguments, &mut frames)?,
                EvaluationFrame::AliasTarget {
                    key,
                    definition,
                    target,
                } => {
                    let normalized = self
                        .memo
                        .get(&target)
                        .copied()
                        .ok_or(TypeNormalizationError::MissingAlias(definition))?;
                    self.complete(key, normalized);
                    let popped = self.alias_stack.pop();
                    debug_assert_eq!(popped, Some(definition));
                }
            }
        }
        self.memo
            .get(&root)
            .copied()
            .ok_or(TypeNormalizationError::InvalidBoundType(ty))
    }

    fn enter(
        &mut self,
        key: EvaluationKey,
        frames: &mut Vec<EvaluationFrame>,
    ) -> Result<(), TypeNormalizationError> {
        if self.memo.contains_key(&key) {
            return Ok(());
        }
        let kind = self
            .kinds
            .get(key.ty.index())
            .cloned()
            .ok_or(TypeNormalizationError::InvalidBoundType(key.ty))?;
        if !self.active.insert(key.clone()) {
            let repeated = match kind {
                BoundTypeKind::Alias { definition, .. } => Some(definition),
                _ => self.alias_stack.iter().rev().copied().find(|alias| {
                    self.context.aliases.get(alias).is_some_and(|definition| {
                        definition.declaration == key.declaration && definition.target == key.ty
                    })
                }),
            }
            .ok_or(TypeNormalizationError::InvalidBoundType(key.ty))?;
            return Err(self.recursive_alias(repeated)?);
        }
        match kind {
            BoundTypeKind::Builtin(builtin) => {
                let normalized = self.store.builtin(builtin);
                self.complete(key, normalized);
            }
            BoundTypeKind::GenericParameter(parameter) => {
                let normalized = if let Some(substitution) = key
                    .substitutions
                    .iter()
                    .find_map(|(candidate, ty)| (*candidate == parameter).then_some(*ty))
                {
                    substitution
                } else {
                    self.store
                        .intern(TypeKind::GenericParameter(parameter))
                        .map_err(|_| TypeNormalizationError::InconsistentTypeStore)?
                };
                self.complete(key, normalized);
            }
            BoundTypeKind::SelfType(owner) => {
                let normalized = self
                    .context
                    .self_types
                    .get(&owner)
                    .copied()
                    .ok_or(TypeNormalizationError::InvalidSelf(owner))?;
                self.complete(key, normalized);
            }
            BoundTypeKind::Alias {
                definition,
                arguments,
            } => {
                self.alias_stack.push(definition);
                let arguments: Vec<_> = arguments
                    .iter()
                    .copied()
                    .map(|argument| key.child(argument))
                    .collect();
                frames.push(EvaluationFrame::AliasArguments {
                    key,
                    definition,
                    arguments: arguments.clone(),
                });
                for argument in arguments.iter().rev() {
                    frames.push(EvaluationFrame::Enter(argument.clone()));
                }
            }
            other => {
                let dependencies = dependencies(&key, &other);
                frames.push(EvaluationFrame::Finish { key, kind: other });
                for dependency in dependencies.into_iter().rev() {
                    frames.push(EvaluationFrame::Enter(dependency));
                }
            }
        }
        Ok(())
    }

    fn expand_alias(
        &mut self,
        key: EvaluationKey,
        definition: TypeAliasId,
        arguments: &[EvaluationKey],
        frames: &mut Vec<EvaluationFrame>,
    ) -> Result<(), TypeNormalizationError> {
        let alias = self
            .context
            .aliases
            .get(&definition)
            .ok_or(TypeNormalizationError::MissingAlias(definition))?;
        if alias.parameters.len() != arguments.len() {
            return Err(TypeNormalizationError::MissingAlias(definition));
        }
        let substitutions = alias
            .parameters
            .iter()
            .copied()
            .zip(arguments.iter().map(|argument| {
                self.memo
                    .get(argument)
                    .copied()
                    .ok_or(TypeNormalizationError::InvalidBoundType(argument.ty))
            }))
            .map(|(parameter, argument)| argument.map(|argument| (parameter, argument)))
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        let target = EvaluationKey {
            ty: alias.target,
            declaration: alias.declaration,
            substitutions,
        };
        frames.push(EvaluationFrame::AliasTarget {
            key,
            definition,
            target: target.clone(),
        });
        frames.push(EvaluationFrame::Enter(target));
        Ok(())
    }

    fn finish(
        &mut self,
        key: EvaluationKey,
        kind: BoundTypeKind,
    ) -> Result<(), TypeNormalizationError> {
        let normalized = match kind {
            BoundTypeKind::Nominal {
                definition,
                arguments,
            } => TypeKind::Nominal {
                definition,
                arguments: self.results(&key, &arguments)?,
            },
            BoundTypeKind::Opaque {
                definition,
                arguments,
            } => TypeKind::Opaque {
                definition,
                arguments: self.results(&key, &arguments)?,
            },
            BoundTypeKind::AssociatedSelection { base, name } => {
                let bound_base = base;
                let base = self.result(&key, bound_base)?;
                let associated =
                    self.resolve_associated(key.declaration, key.ty, bound_base, base, name)?;
                TypeKind::AssociatedProjection { base, associated }
            }
            BoundTypeKind::Pointer(pointee) => TypeKind::Pointer(self.result(&key, pointee)?),
            BoundTypeKind::Borrow {
                capability,
                referent,
            } => TypeKind::Borrow {
                capability,
                referent: self.result(&key, referent)?,
            },
            BoundTypeKind::Slice(element) => TypeKind::Slice(self.result(&key, element)?),
            BoundTypeKind::FixedArray { element, length } => TypeKind::FixedArray {
                element: self.result(&key, element)?,
                length: self
                    .array_lengths
                    .get(&length)
                    .copied()
                    .ok_or(TypeNormalizationError::InvalidBoundType(key.ty))?,
            },
            BoundTypeKind::Callable(callable) => {
                let mut parameters = self.results(&key, callable.parameters())?.into_vec();
                let result = self.result(&key, callable.result())?;
                let provenance = match callable.explicit_origins() {
                    Some(origins) => ResultProvenance::from_origins(origins.iter().copied())
                        .map_err(|_| TypeNormalizationError::InvalidBoundType(key.ty))?,
                    None => self.infer_callable_provenance(
                        key.ty,
                        &parameters,
                        callable.named_parameters(),
                        result,
                    )?,
                };
                let pack = if callable.has_argument_pack() {
                    parameters.pop()
                } else {
                    None
                };
                TypeKind::Callable(
                    CallableContract::new(
                        callable.capability(),
                        parameters,
                        pack,
                        result,
                        provenance,
                    )
                    .map_err(|_| TypeNormalizationError::InvalidBoundType(key.ty))?,
                )
            }
            BoundTypeKind::Optional(payload) => TypeKind::Optional(self.result(&key, payload)?),
            BoundTypeKind::Fallible(success) => TypeKind::Fallible(self.result(&key, success)?),
            BoundTypeKind::Builtin(_)
            | BoundTypeKind::GenericParameter(_)
            | BoundTypeKind::SelfType(_)
            | BoundTypeKind::Alias { .. } => {
                return Err(TypeNormalizationError::InvalidBoundType(key.ty));
            }
        };
        let normalized = self
            .store
            .intern(normalized)
            .map_err(|_| TypeNormalizationError::InconsistentTypeStore)?;
        self.complete(key, normalized);
        Ok(())
    }

    fn result(
        &self,
        parent: &EvaluationKey,
        child: BoundTypeId,
    ) -> Result<TypeId, TypeNormalizationError> {
        self.memo
            .get(&parent.child(child))
            .copied()
            .ok_or(TypeNormalizationError::InvalidBoundType(child))
    }

    fn results(
        &self,
        parent: &EvaluationKey,
        children: &[BoundTypeId],
    ) -> Result<Box<[TypeId]>, TypeNormalizationError> {
        children
            .iter()
            .copied()
            .map(|child| self.result(parent, child))
            .collect()
    }

    fn complete(&mut self, key: EvaluationKey, normalized: TypeId) {
        self.active.remove(&key);
        self.memo.insert(key, normalized);
    }

    fn infer_callable_provenance(
        &self,
        bound: BoundTypeId,
        parameters: &[TypeId],
        named: &[bool],
        result: TypeId,
    ) -> Result<ResultProvenance, TypeNormalizationError> {
        if !self.store.may_carry_storage(result) {
            return Ok(ResultProvenance::empty());
        }
        let eligible: Vec<_> = parameters
            .iter()
            .enumerate()
            .filter(|(position, ty)| {
                named.get(*position).copied().unwrap_or(false) && self.store.may_carry_storage(**ty)
            })
            .map(|(position, _)| ParameterOrigin::new(position))
            .collect();
        let unnamed_eligible = parameters.iter().enumerate().any(|(position, ty)| {
            !named.get(position).copied().unwrap_or(false) && self.store.may_carry_storage(*ty)
        });
        match eligible.as_slice() {
            [] if !unnamed_eligible => Ok(ResultProvenance::empty()),
            [origin] if !unnamed_eligible => ResultProvenance::from_origins([*origin])
                .map_err(|_| TypeNormalizationError::InvalidBoundType(bound)),
            _ => Err(
                self.authored_violation(TypeNormalizationRule::AmbiguousCallableProvenance, bound)?
            ),
        }
    }

    fn resolve_associated(
        &mut self,
        declaration: SurfaceDeclarationId,
        selection: BoundTypeId,
        bound_base: BoundTypeId,
        base: TypeId,
        name: Symbol,
    ) -> Result<AssociatedTypeId, TypeNormalizationError> {
        let mut candidates = HashSet::new();
        match self.kinds.get(bound_base.index()) {
            Some(BoundTypeKind::GenericParameter(parameter)) => {
                self.collect_parameter_associated(declaration, *parameter, name, &mut candidates);
            }
            _ => match self.store.get(base) {
                Some(TypeKind::InterfaceSelf(interface)) => {
                    if let Some(associated) = self.context.associated.get(&(*interface, name)) {
                        candidates.insert(*associated);
                    }
                }
                Some(TypeKind::GenericParameter(parameter)) => self.collect_parameter_associated(
                    declaration,
                    *parameter,
                    name,
                    &mut candidates,
                ),
                Some(TypeKind::AssociatedProjection { associated, .. }) => {
                    if let Some(surface) = self.context.associated_surfaces.get(associated) {
                        self.collect_subject_associated(
                            *surface,
                            RequirementSubject::AssociatedType(*associated),
                            name,
                            &mut candidates,
                        );
                    }
                }
                Some(_) => {
                    self.collect_interface_implementation_associated(base, name, &mut candidates);
                }
                None => {
                    return Err(TypeNormalizationError::InconsistentTypeStore);
                }
            },
        }
        match candidates.len() {
            0 => {
                Err(self
                    .authored_violation(TypeNormalizationRule::UnknownAssociatedType, selection)?)
            }
            1 => {
                let associated = candidates.iter().next().copied().ok_or(
                    TypeNormalizationError::InconsistentAssociatedIndex(declaration),
                )?;
                let origin = self
                    .origins
                    .bound(selection)
                    .ok_or(TypeNormalizationError::InvalidBoundType(selection))?;
                self.associated_projection_uses
                    .push(AssociatedProjectionUse::new(base, associated, origin));
                Ok(associated)
            }
            _ => Err(
                self.authored_violation(TypeNormalizationRule::AmbiguousAssociatedType, selection)?
            ),
        }
    }

    fn authored_violation(
        &self,
        rule: TypeNormalizationRule,
        ty: BoundTypeId,
    ) -> Result<TypeNormalizationError, TypeNormalizationError> {
        let origin = self
            .origins
            .bound(ty)
            .ok_or(TypeNormalizationError::InvalidBoundType(ty))?;
        Ok(TypeNormalizationViolation::new(rule, origin).into())
    }

    fn recursive_alias(
        &self,
        repeated: TypeAliasId,
    ) -> Result<TypeNormalizationError, TypeNormalizationError> {
        let start = self
            .alias_stack
            .iter()
            .position(|alias| *alias == repeated)
            .ok_or(TypeNormalizationError::MissingAlias(repeated))?;
        let mut cycle = self.alias_stack[start..].to_vec();
        let rotation = cycle
            .iter()
            .enumerate()
            .min_by_key(|(_, alias)| {
                self.context
                    .aliases
                    .get(alias)
                    .map_or(usize::MAX, |definition| definition.declaration.index())
            })
            .map_or(0, |(index, _)| index);
        cycle.rotate_left(rotation);
        let origins = cycle
            .into_iter()
            .map(|alias| {
                self.origins
                    .alias(alias)
                    .ok_or(TypeNormalizationError::MissingAlias(alias))
            })
            .collect::<Result<Vec<_>, _>>()?;
        TypeNormalizationViolation::alias_cycle(origins)
            .map(TypeNormalizationError::from)
            .ok_or(TypeNormalizationError::MissingAlias(repeated))
    }

    fn collect_parameter_associated(
        &self,
        mut declaration: SurfaceDeclarationId,
        parameter: GenericParameterId,
        name: Symbol,
        candidates: &mut HashSet<AssociatedTypeId>,
    ) {
        loop {
            self.collect_subject_associated(
                declaration,
                RequirementSubject::GenericParameter(parameter),
                name,
                candidates,
            );
            let Some(owner) = self
                .context
                .declarations
                .get(declaration.index())
                .and_then(|surface| surface.owner())
            else {
                break;
            };
            declaration = owner;
        }
    }

    fn collect_subject_associated(
        &self,
        declaration: SurfaceDeclarationId,
        subject: RequirementSubject,
        name: Symbol,
        candidates: &mut HashSet<AssociatedTypeId>,
    ) {
        let Some(requirements) = self.context.bound_requirements.get(declaration.index()) else {
            return;
        };
        for requirement in requirements {
            let BoundRequirementKind::Interface {
                subject: candidate,
                application,
                ..
            } = requirement
            else {
                continue;
            };
            if *candidate == subject
                && let Some(associated) =
                    self.context.associated.get(&(application.definition, name))
            {
                candidates.insert(*associated);
            }
        }
    }

    fn collect_interface_implementation_associated(
        &self,
        base: TypeId,
        name: Symbol,
        candidates: &mut HashSet<AssociatedTypeId>,
    ) {
        for (declaration, interface) in &self.context.implementation_interfaces {
            let Some(owner) = self.context.declarations[declaration.index()].owner() else {
                continue;
            };
            let Some(target) = self.context.patterns[owner.index()].first() else {
                continue;
            };
            if same_attachment_family(self.store, target, base)
                && let Some(associated) = self.context.associated.get(&(*interface, name))
            {
                candidates.insert(*associated);
            }
        }
    }
}

fn dependencies(key: &EvaluationKey, kind: &BoundTypeKind) -> Vec<EvaluationKey> {
    let children: Vec<_> = match kind {
        BoundTypeKind::Nominal { arguments, .. } | BoundTypeKind::Opaque { arguments, .. } => {
            arguments.to_vec()
        }
        BoundTypeKind::AssociatedSelection { base, .. }
        | BoundTypeKind::Pointer(base)
        | BoundTypeKind::Borrow { referent: base, .. }
        | BoundTypeKind::Slice(base)
        | BoundTypeKind::FixedArray { element: base, .. }
        | BoundTypeKind::Optional(base)
        | BoundTypeKind::Fallible(base) => vec![*base],
        BoundTypeKind::Callable(callable) => callable
            .parameters()
            .iter()
            .copied()
            .chain([callable.result()])
            .collect(),
        BoundTypeKind::Builtin(_)
        | BoundTypeKind::GenericParameter(_)
        | BoundTypeKind::SelfType(_)
        | BoundTypeKind::Alias { .. } => Vec::new(),
    };
    children.into_iter().map(|child| key.child(child)).collect()
}

/// Compares only the declaration attachment family used to discover a member identity.
///
/// This is deliberately not interface applicability. Refinements, requirements, and overlap are
/// checked later by `InterfaceImplementationTable`, after every declaration is canonical.
fn same_attachment_family(
    store: &TypeStore,
    pattern: &NormalizedDeclarationPattern,
    candidate: TypeId,
) -> bool {
    match (pattern, store.get(candidate)) {
        (NormalizedDeclarationPattern::Type(expected), _) if *expected == candidate => true,
        (
            NormalizedDeclarationPattern::Type(expected),
            Some(TypeKind::Nominal { definition, .. }),
        ) => matches!(
            store.get(*expected),
            Some(TypeKind::Nominal {
                definition: expected,
                ..
            }) if expected == definition
        ),
        (NormalizedDeclarationPattern::Type(expected), Some(TypeKind::Slice(_))) => {
            matches!(store.get(*expected), Some(TypeKind::Slice(_)))
        }
        _ => false,
    }
}

/// Converts the bound header arena into canonical structural types.
///
/// Alias applications disappear during this pass. Associated selections resolve to exact
/// associated declaration identities, and structural callable names remain absent from type
/// identity after their result-origin positions have been established.
///
/// # Errors
///
/// Returns an error for recursive aliases, invalid `Self`, unknown or ambiguous associated
/// selections, inconsistent bound graphs, or a bodyless structural callable whose omitted
/// provenance cannot be inferred uniquely.
#[allow(clippy::too_many_lines)] // One deterministic arena-freezing pass is clearer kept together.
pub fn normalize_header_types(
    bindings: PreparedTypeBindings<'_>,
) -> Result<PreparedTypes<'_>, TypeNormalizationError> {
    let PreparedTypeBindings {
        mut namespaces,
        kinds,
        roots: bound_roots,
        root_declarations,
        alias_targets: bound_alias_targets,
        patterns: bound_patterns,
        interface_applications: bound_interface_applications,
        interface_application_declarations,
        opaque_results: bound_opaque_results,
        callable_results: bound_callable_results,
        requirements: bound_requirements,
        normalization_origins,
        constant_values,
        array_lengths,
    } = bindings;
    let context = prepare_context(
        &mut namespaces,
        &bound_alias_targets,
        &bound_patterns,
        &bound_interface_applications,
        &interface_application_declarations,
        bound_requirements,
    )?;
    let store = namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .types_mut();
    let mut evaluator = Evaluator {
        kinds: &kinds,
        origins: &normalization_origins,
        context: &context,
        store,
        memo: HashMap::new(),
        active: HashSet::new(),
        alias_stack: Vec::new(),
        array_lengths: &array_lengths,
        associated_projection_uses: Vec::new(),
    };

    let mut ordered_roots: Vec<_> = bound_roots.into_iter().collect();
    ordered_roots.sort_by_key(|(node, _)| (node.source(), node.index()));
    let mut roots = HashMap::with_capacity(ordered_roots.len());
    for (node, bound) in ordered_roots {
        let declaration = root_declarations
            .get(&node)
            .copied()
            .ok_or(TypeNormalizationError::InvalidBoundType(bound))?;
        roots.insert(node, evaluator.normalize(bound, declaration)?);
    }

    let mut alias_targets = HashMap::new();
    let mut ordered_aliases: Vec<_> = context.aliases.iter().collect();
    ordered_aliases.sort_by_key(|(_, alias)| alias.declaration.index());
    for (alias, definition) in ordered_aliases {
        alias_targets.insert(
            *alias,
            evaluator.normalize(definition.target, definition.declaration)?,
        );
    }

    let mut interface_applications = HashMap::new();
    let mut ordered_interface_applications: Vec<_> =
        bound_interface_applications.into_iter().collect();
    ordered_interface_applications.sort_by_key(|(node, _)| (node.source(), node.index()));
    for (node, application) in ordered_interface_applications {
        let declaration = interface_application_declarations
            .get(&node)
            .copied()
            .ok_or(TypeNormalizationError::MissingInterfaceApplicationContext(
                node,
            ))?;
        interface_applications.insert(
            node,
            normalize_interface_application(&mut evaluator, declaration, &application)?,
        );
    }

    let callable_results = bound_callable_results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            result
                .map(|result| evaluator.normalize(result, SurfaceDeclarationId::from_index(index)))
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    let opaque_results = normalize_opaque_results(&mut evaluator, &bound_opaque_results)?;

    let mut requirements = Vec::with_capacity(context.bound_requirements.len());
    for (index, bound) in context.bound_requirements.iter().enumerate() {
        let declaration = SurfaceDeclarationId::from_index(index);
        let mut normalized = Vec::with_capacity(bound.len());
        let mut interface_requirements = HashMap::new();
        for bound_requirement in bound {
            let requirement =
                normalize_requirement(&mut evaluator, declaration, bound_requirement)?;
            if let (
                BoundRequirementKind::Interface { origin, .. },
                RequirementKind::Interface {
                    subject,
                    application,
                    ..
                },
            ) = (bound_requirement, &requirement)
            {
                let key = (*subject, application.clone());
                if let Some(first) = interface_requirements.insert(key, *origin) {
                    return Err(TypeNormalizationError::RequirementRule(
                        TypeBindingViolation::duplicate(
                            TypeBindingRule::DuplicateInterfaceRequirement,
                            first,
                            *origin,
                        ),
                    ));
                }
            }
            normalized.push(requirement);
        }
        requirements.push(normalized.into_boxed_slice());
    }

    let associated_projection_uses = std::mem::take(&mut evaluator.associated_projection_uses);
    drop(evaluator);

    Ok(PreparedTypes {
        namespaces,
        roots,
        alias_targets,
        patterns: context.patterns.clone(),
        interface_applications,
        opaque_results,
        callable_results,
        requirements: requirements.into_boxed_slice(),
        constant_values,
        associated_projection_uses: associated_projection_uses.into_boxed_slice(),
    })
}

fn normalize_opaque_results(
    evaluator: &mut Evaluator<'_>,
    bound: &HashMap<OpaqueTypeId, BoundOpaqueResult>,
) -> Result<HashMap<OpaqueTypeId, NormalizedOpaqueResult>, TypeNormalizationError> {
    let mut ordered = bound
        .iter()
        .map(|(opaque, result)| {
            evaluator
                .context
                .entity_declarations
                .get(&ReservedEntity::OpaqueType(*opaque))
                .copied()
                .map(|declaration| (declaration, *opaque, result))
                .ok_or(TypeNormalizationError::InvalidSelf(
                    ReservedEntity::OpaqueType(*opaque),
                ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    ordered.sort_unstable_by_key(|(declaration, _, _)| *declaration);
    let mut normalized = HashMap::with_capacity(ordered.len());
    for (declaration, opaque, result) in ordered {
        let arguments = result
            .arguments
            .iter()
            .map(|argument| evaluator.normalize(*argument, declaration))
            .collect::<Result<Vec<_>, _>>()?;
        let associated_types = result
            .associated_types
            .iter()
            .map(|(associated, ty)| {
                evaluator
                    .normalize(*ty, declaration)
                    .map(|ty| AssociatedTypeBinding::new(*associated, ty))
            })
            .collect::<Result<Vec<_>, _>>()?;
        normalized.insert(
            opaque,
            NormalizedOpaqueResult {
                generic_parameters: result.generic_parameters.clone(),
                interface: InterfaceApplication::new(result.interface, arguments),
                associated_types: associated_types.into_boxed_slice(),
                result: evaluator.normalize(result.result, declaration)?,
            },
        );
    }
    Ok(normalized)
}

fn normalize_interface_application(
    evaluator: &mut Evaluator<'_>,
    declaration: SurfaceDeclarationId,
    application: &BoundInterfaceApplication,
) -> Result<InterfaceApplication, TypeNormalizationError> {
    Ok(InterfaceApplication::new(
        application.definition,
        application
            .arguments
            .iter()
            .map(|argument| evaluator.normalize(*argument, declaration))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn normalize_requirement(
    evaluator: &mut Evaluator<'_>,
    declaration: SurfaceDeclarationId,
    requirement: &BoundRequirementKind,
) -> Result<RequirementKind, TypeNormalizationError> {
    Ok(match requirement {
        BoundRequirementKind::Interface {
            subject,
            application,
            associated_types,
            ..
        } => RequirementKind::Interface {
            subject: *subject,
            application: normalize_interface_application(evaluator, declaration, application)?,
            associated_types: associated_types
                .iter()
                .map(|binding| {
                    let projection = evaluator.normalize(binding.projection, declaration)?;
                    let Some(TypeKind::AssociatedProjection { associated, .. }) =
                        evaluator.store.get(projection)
                    else {
                        return Err(TypeNormalizationError::InvalidBoundType(binding.projection));
                    };
                    Ok(AssociatedTypeBinding::new(
                        *associated,
                        evaluator.normalize(binding.value, declaration)?,
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        },
        BoundRequirementKind::Callable { subject, contract } => {
            let ty = evaluator.normalize(*contract, declaration)?;
            let Some(TypeKind::Callable(contract)) = evaluator.store.get(ty) else {
                return Err(evaluator.authored_violation(
                    TypeNormalizationRule::InvalidCallableRequirement,
                    *contract,
                )?);
            };
            RequirementKind::Callable {
                subject: *subject,
                contract: contract.clone(),
            }
        }
        BoundRequirementKind::Copy(parameter) => RequirementKind::Copy(*parameter),
        BoundRequirementKind::Equality { operand } => {
            RequirementKind::Equality { operand: *operand }
        }
        BoundRequirementKind::Ordering { operand } => {
            RequirementKind::Ordering { operand: *operand }
        }
        BoundRequirementKind::Index {
            capability,
            container,
            index,
            result,
        } => RequirementKind::Index {
            capability: *capability,
            container: *container,
            index: evaluator.normalize(*index, declaration)?,
            result: evaluator.normalize(*result, declaration)?,
        },
        BoundRequirementKind::Coercion { source, target } => RequirementKind::Coercion {
            source: evaluator.normalize(*source, declaration)?,
            target: evaluator.normalize(*target, declaration)?,
        },
        BoundRequirementKind::Expansion {
            capability,
            source,
            result,
        } => RequirementKind::Expansion {
            capability: *capability,
            source: *source,
            result: evaluator.normalize(*result, declaration)?,
        },
        BoundRequirementKind::BinderRefinement {
            parameter,
            replacement,
            ..
        } => RequirementKind::BinderRefinement {
            parameter: *parameter,
            replacement: evaluator.normalize(*replacement, declaration)?,
        },
    })
}
