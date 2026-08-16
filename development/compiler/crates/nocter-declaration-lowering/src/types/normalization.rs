use std::collections::{HashMap, HashSet};
use std::fmt;

use nocter_declarations::{
    InterfaceApplication, RequirementKind, RequirementSubject, StructuralCapability,
};
use nocter_model::{
    AssociatedTypeId, BuiltinType, CallableContract, GenericParameterId, InterfaceId,
    ParameterOrigin, ResultProvenance, Symbol, TypeAliasId, TypeId, TypeKind, TypeStore,
};
use nocter_syntax::NodeId;

use crate::{PreparedNamespaces, ReservedEntity, SurfaceDeclaration, SurfaceDeclarationId};

use super::{
    BoundCapability, BoundRequirementKind, BoundTypeId, BoundTypeKind, PreparedTypeBindings,
};

mod preparation;

use preparation::prepare_context;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NormalizedDeclarationPattern {
    Type(TypeId),
    Interface(InterfaceApplication),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeNormalizationError {
    InvalidBoundType(BoundTypeId),
    InconsistentTypeStore,
    MissingCapabilityContext(NodeId),
    MissingAlias(TypeAliasId),
    RecursiveAlias(TypeAliasId),
    InvalidSelf(ReservedEntity),
    UnknownAssociatedType {
        declaration: SurfaceDeclarationId,
        name: Symbol,
    },
    AmbiguousAssociatedType {
        declaration: SurfaceDeclarationId,
        name: Symbol,
    },
    AmbiguousCallableProvenance(BoundTypeId),
}

impl fmt::Display for TypeNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBoundType(ty) => write!(formatter, "bound type {ty:?} is inconsistent"),
            Self::InconsistentTypeStore => {
                formatter.write_str("normalized type store contains an invalid reference")
            }
            Self::MissingCapabilityContext(node) => {
                write!(formatter, "capability {node:?} has no declaration context")
            }
            Self::MissingAlias(alias) => write!(formatter, "type alias {alias:?} has no target"),
            Self::RecursiveAlias(alias) => {
                write!(formatter, "type alias {alias:?} expands recursively")
            }
            Self::InvalidSelf(owner) => write!(formatter, "{owner:?} has no normalized Self type"),
            Self::UnknownAssociatedType { declaration, name } => write!(
                formatter,
                "type selection in {declaration:?} names unknown associated type {name:?}"
            ),
            Self::AmbiguousAssociatedType { declaration, name } => write!(
                formatter,
                "type selection in {declaration:?} ambiguously names associated type {name:?}"
            ),
            Self::AmbiguousCallableProvenance(ty) => write!(
                formatter,
                "structural callable {ty:?} requires explicit result provenance"
            ),
        }
    }
}

impl std::error::Error for TypeNormalizationError {}

/// Header types after aliases, `Self`, associated names, and requirement types are canonicalized.
#[derive(Debug)]
pub struct PreparedTypes<'syntax> {
    namespaces: PreparedNamespaces<'syntax>,
    roots: HashMap<NodeId, TypeId>,
    alias_targets: HashMap<TypeAliasId, TypeId>,
    patterns: Box<[Box<[NormalizedDeclarationPattern]>]>,
    capabilities: HashMap<NodeId, StructuralCapability>,
    requirements: Box<[Box<[RequirementKind]>]>,
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
    pub fn capability_for(&self, node: NodeId) -> Option<&StructuralCapability> {
        self.capabilities.get(&node)
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
    entities: Box<[Option<ReservedEntity>]>,
    aliases: HashMap<TypeAliasId, AliasDefinition>,
    associated: HashMap<(InterfaceId, Symbol), AssociatedTypeId>,
    associated_surfaces: HashMap<AssociatedTypeId, SurfaceDeclarationId>,
    self_types: HashMap<ReservedEntity, TypeId>,
    patterns: Box<[Box<[NormalizedDeclarationPattern]>]>,
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
    context: &'a NormalizationContext,
    store: &'a mut TypeStore,
    memo: HashMap<EvaluationKey, TypeId>,
    active: HashSet<EvaluationKey>,
    alias_stack: Vec<TypeAliasId>,
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
            return Err(self
                .alias_stack
                .last()
                .copied()
                .map(TypeNormalizationError::RecursiveAlias)
                .unwrap_or(TypeNormalizationError::InvalidBoundType(key.ty)));
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
            BoundTypeKind::AssociatedSelection { base, name } => {
                let base = self.result(&key, base)?;
                let associated = self.resolve_associated(key.declaration, base, name)?;
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
                length,
            },
            BoundTypeKind::Callable(callable) => {
                let parameters = self.results(&key, callable.parameters())?;
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
                TypeKind::Callable(
                    CallableContract::new(callable.capability(), parameters, result, provenance)
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
        if !may_carry_storage(self.store, result) {
            return Ok(ResultProvenance::empty());
        }
        let eligible: Vec<_> = parameters
            .iter()
            .enumerate()
            .filter(|(position, ty)| {
                named.get(*position).copied().unwrap_or(false)
                    && may_carry_storage(self.store, **ty)
            })
            .map(|(position, _)| ParameterOrigin::new(position))
            .collect();
        let unnamed_eligible = parameters.iter().enumerate().any(|(position, ty)| {
            !named.get(position).copied().unwrap_or(false) && may_carry_storage(self.store, *ty)
        });
        match eligible.as_slice() {
            [] if !unnamed_eligible => Ok(ResultProvenance::empty()),
            [origin] if !unnamed_eligible => ResultProvenance::from_origins([*origin])
                .map_err(|_| TypeNormalizationError::AmbiguousCallableProvenance(bound)),
            _ => Err(TypeNormalizationError::AmbiguousCallableProvenance(bound)),
        }
    }

    fn resolve_associated(
        &self,
        declaration: SurfaceDeclarationId,
        base: TypeId,
        name: Symbol,
    ) -> Result<AssociatedTypeId, TypeNormalizationError> {
        let mut candidates = HashSet::new();
        let occurrences = match self.store.get(base) {
            Some(TypeKind::InterfaceSelf(interface)) => {
                if let Some(associated) = self.context.associated.get(&(*interface, name)) {
                    candidates.insert(*associated);
                    1
                } else {
                    0
                }
            }
            Some(TypeKind::GenericParameter(parameter)) => {
                self.collect_parameter_associated(declaration, *parameter, name, &mut candidates)
            }
            Some(TypeKind::AssociatedProjection { associated, .. }) => {
                if let Some(surface) = self.context.associated_surfaces.get(associated) {
                    self.collect_subject_associated(
                        *surface,
                        RequirementSubject::AssociatedType(*associated),
                        name,
                        &mut candidates,
                    )
                } else {
                    0
                }
            }
            Some(_) => self.collect_conformance_associated(base, name, &mut candidates),
            None => {
                return Err(TypeNormalizationError::InconsistentTypeStore);
            }
        };
        match occurrences {
            0 => Err(TypeNormalizationError::UnknownAssociatedType { declaration, name }),
            1 if candidates.len() == 1 => candidates
                .iter()
                .next()
                .copied()
                .ok_or(TypeNormalizationError::UnknownAssociatedType { declaration, name }),
            _ => Err(TypeNormalizationError::AmbiguousAssociatedType { declaration, name }),
        }
    }

    fn collect_parameter_associated(
        &self,
        mut declaration: SurfaceDeclarationId,
        parameter: GenericParameterId,
        name: Symbol,
        candidates: &mut HashSet<AssociatedTypeId>,
    ) -> usize {
        let mut occurrences = 0;
        loop {
            occurrences += self.collect_subject_associated(
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
        occurrences
    }

    fn collect_subject_associated(
        &self,
        declaration: SurfaceDeclarationId,
        subject: RequirementSubject,
        name: Symbol,
        candidates: &mut HashSet<AssociatedTypeId>,
    ) -> usize {
        let Some(requirements) = self.context.bound_requirements.get(declaration.index()) else {
            return 0;
        };
        let mut occurrences = 0;
        for requirement in requirements {
            let BoundRequirementKind::Capability {
                subject: candidate,
                capability:
                    BoundCapability::Interface {
                        definition,
                        arguments: _,
                    },
            } = requirement
            else {
                continue;
            };
            if *candidate == subject
                && let Some(associated) = self.context.associated.get(&(*definition, name))
            {
                candidates.insert(*associated);
                occurrences += 1;
            }
        }
        occurrences
    }

    fn collect_conformance_associated(
        &self,
        base: TypeId,
        name: Symbol,
        candidates: &mut HashSet<AssociatedTypeId>,
    ) -> usize {
        let mut occurrences = 0;
        for (index, patterns) in self.context.patterns.iter().enumerate() {
            let Some(ReservedEntity::Conformance(_)) = self.context.entities[index] else {
                continue;
            };
            let [NormalizedDeclarationPattern::Interface(interface), target] = patterns.as_ref()
            else {
                continue;
            };
            if pattern_matches(self.store, target, base)
                && let Some(associated) =
                    self.context.associated.get(&(interface.interface(), name))
            {
                candidates.insert(*associated);
                occurrences += 1;
            }
        }
        occurrences
    }
}

fn dependencies(key: &EvaluationKey, kind: &BoundTypeKind) -> Vec<EvaluationKey> {
    let children: Vec<_> = match kind {
        BoundTypeKind::Nominal { arguments, .. } => arguments.to_vec(),
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

fn may_carry_storage(store: &TypeStore, root: TypeId) -> bool {
    let mut pending = vec![root];
    let mut visited = HashSet::new();
    while let Some(ty) = pending.pop() {
        if !visited.insert(ty) {
            continue;
        }
        match store.get(ty) {
            Some(
                TypeKind::Builtin(BuiltinType::Str | BuiltinType::Error)
                | TypeKind::GenericParameter(_)
                | TypeKind::InterfaceSelf(_)
                | TypeKind::Nominal { .. }
                | TypeKind::AssociatedProjection { .. }
                | TypeKind::Opaque { .. }
                | TypeKind::Pointer(_)
                | TypeKind::Borrow { .. }
                | TypeKind::Slice(_)
                | TypeKind::Callable(_),
            ) => return true,
            Some(
                TypeKind::FixedArray { element, .. }
                | TypeKind::Optional(element)
                | TypeKind::Fallible(element),
            ) => {
                pending.push(*element);
            }
            Some(TypeKind::Builtin(_)) | None => {}
        }
    }
    false
}

fn pattern_matches(
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
        capabilities: bound_capabilities,
        capability_declarations,
        requirements: bound_requirements,
    } = bindings;
    let context = prepare_context(
        &mut namespaces,
        &bound_alias_targets,
        &bound_patterns,
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
        context: &context,
        store,
        memo: HashMap::new(),
        active: HashSet::new(),
        alias_stack: Vec::new(),
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

    let mut capabilities = HashMap::new();
    let mut ordered_capabilities: Vec<_> = bound_capabilities.into_iter().collect();
    ordered_capabilities.sort_by_key(|(node, _)| (node.source(), node.index()));
    for (node, capability) in ordered_capabilities {
        let declaration = capability_declarations
            .get(&node)
            .copied()
            .ok_or(TypeNormalizationError::MissingCapabilityContext(node))?;
        capabilities.insert(
            node,
            normalize_capability(&mut evaluator, declaration, &capability)?,
        );
    }

    let mut requirements = Vec::with_capacity(context.bound_requirements.len());
    for (index, bound) in context.bound_requirements.iter().enumerate() {
        let declaration = SurfaceDeclarationId::from_index(index);
        requirements.push(
            bound
                .iter()
                .map(|requirement| normalize_requirement(&mut evaluator, declaration, requirement))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
        );
    }

    Ok(PreparedTypes {
        namespaces,
        roots,
        alias_targets,
        patterns: context.patterns.clone(),
        capabilities,
        requirements: requirements.into_boxed_slice(),
    })
}

fn normalize_capability(
    evaluator: &mut Evaluator<'_>,
    declaration: SurfaceDeclarationId,
    capability: &BoundCapability,
) -> Result<StructuralCapability, TypeNormalizationError> {
    match capability {
        BoundCapability::Interface {
            definition,
            arguments,
        } => Ok(StructuralCapability::Interface(InterfaceApplication::new(
            *definition,
            arguments
                .iter()
                .map(|argument| evaluator.normalize(*argument, declaration))
                .collect::<Result<Vec<_>, _>>()?,
        ))),
        BoundCapability::Callable(callable) => {
            let ty = evaluator.normalize(*callable, declaration)?;
            let Some(TypeKind::Callable(contract)) = evaluator.store.get(ty) else {
                return Err(TypeNormalizationError::InvalidBoundType(*callable));
            };
            Ok(StructuralCapability::Callable(contract.clone()))
        }
    }
}

fn normalize_requirement(
    evaluator: &mut Evaluator<'_>,
    declaration: SurfaceDeclarationId,
    requirement: &BoundRequirementKind,
) -> Result<RequirementKind, TypeNormalizationError> {
    Ok(match requirement {
        BoundRequirementKind::Capability {
            subject,
            capability,
        } => RequirementKind::Capability {
            subject: *subject,
            capability: normalize_capability(evaluator, declaration, capability)?,
        },
        BoundRequirementKind::Copy(parameter) => RequirementKind::Copy(*parameter),
        BoundRequirementKind::TypeEquality { left, right } => RequirementKind::TypeEquality {
            left: evaluator.normalize(*left, declaration)?,
            right: evaluator.normalize(*right, declaration)?,
        },
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
        } => RequirementKind::BinderRefinement {
            parameter: *parameter,
            replacement: evaluator.normalize(*replacement, declaration)?,
        },
    })
}
