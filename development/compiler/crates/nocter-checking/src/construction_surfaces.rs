use std::collections::BTreeMap;

use nocter_declarations::{
    CallableKind, CallableOwner, DeclarationGraph, LiteralShape, NominalShape, Visibility,
};
use nocter_model::{
    BuiltinType, CallableId, ConstructionId, FieldId, ModuleId, NominalTypeId, Symbol, TypeId,
    TypeStore, VariantId,
};

use crate::type_relations::InherentTypeFamily;

/// The source-independent entries owned by one constructible type family.
#[derive(Debug)]
struct ConstructionSurface {
    declaration: Option<ConstructionId>,
    structural: Option<StructuralSurface>,
    variants: BTreeMap<Symbol, VariantId>,
    functions: BTreeMap<Symbol, CallableId>,
    literals: BTreeMap<LiteralShape, CallableId>,
}

#[derive(Debug)]
struct StructuralSurface {
    fields: Box<[FieldId]>,
    by_name: BTreeMap<Symbol, FieldId>,
}

/// One selected entry in the compiler-owned construction surface of a nominal type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedConstructionEntry {
    Structural,
    Variant(VariantId),
    Callable(CallableId),
}

/// One ordered view derived from the canonical construction surface.
#[derive(Debug, Eq, PartialEq)]
pub struct SelectedConstructionSurface {
    declaration: Option<ConstructionId>,
    entries: Box<[SelectedConstructionEntry]>,
    default: Option<usize>,
}

impl SelectedConstructionSurface {
    #[must_use]
    pub const fn declaration(&self) -> Option<ConstructionId> {
        self.declaration
    }

    #[must_use]
    pub const fn entries(&self) -> &[SelectedConstructionEntry] {
        &self.entries
    }

    #[must_use]
    pub fn is_default(&self, index: usize) -> bool {
        self.default == Some(index)
    }
}

impl ConstructionSurface {
    fn builtin(declaration: ConstructionId) -> Self {
        Self {
            declaration: Some(declaration),
            structural: None,
            variants: BTreeMap::new(),
            functions: BTreeMap::new(),
            literals: BTreeMap::new(),
        }
    }
}

/// Canonical construction-surface lookup prepared once for the complete program.
///
/// A surface includes structural struct entry, intrinsic enum variants, and an optional authored
/// `construct` declaration. Body checking, editor queries, and later lowering therefore share the
/// same entry identities instead of independently scanning nominal and construction declarations.
#[derive(Debug)]
pub struct ConstructionSurfaceTable {
    by_family: BTreeMap<InherentTypeFamily, ConstructionSurface>,
    by_construction: BTreeMap<ConstructionId, InherentTypeFamily>,
}

impl ConstructionSurfaceTable {
    pub(crate) fn build(
        graph: &DeclarationGraph,
        types: &TypeStore,
    ) -> Result<Self, ConstructionSurfaceBuildError> {
        let mut by_family = BTreeMap::new();
        let mut by_construction = BTreeMap::new();
        for (nominal, declaration) in graph.declarations().nominal_types().iter() {
            let (structural, variants) = match declaration.shape() {
                NominalShape::Struct { fields, .. } => {
                    let mut by_name = BTreeMap::new();
                    for field in fields.iter().copied() {
                        let declaration = graph
                            .declarations()
                            .fields()
                            .get(field)
                            .ok_or(ConstructionSurfaceBuildError::MissingField(field))?;
                        if declaration.owner() != nominal {
                            return Err(ConstructionSurfaceBuildError::InvalidFieldOwner(field));
                        }
                        if by_name.insert(declaration.name(), field).is_some() {
                            return Err(ConstructionSurfaceBuildError::DuplicateFieldName(
                                nominal,
                                declaration.name(),
                            ));
                        }
                    }
                    (
                        Some(StructuralSurface {
                            fields: fields.clone(),
                            by_name,
                        }),
                        BTreeMap::new(),
                    )
                }
                NominalShape::Enum { variants } => {
                    let mut by_name = BTreeMap::new();
                    for variant in variants.iter().copied() {
                        let declaration = graph
                            .declarations()
                            .variants()
                            .get(variant)
                            .ok_or(ConstructionSurfaceBuildError::MissingVariant(variant))?;
                        if declaration.owner() != nominal {
                            return Err(ConstructionSurfaceBuildError::InvalidVariantOwner(
                                variant,
                            ));
                        }
                        if by_name.insert(declaration.name(), variant).is_some() {
                            return Err(ConstructionSurfaceBuildError::DuplicateVariantName(
                                nominal,
                                declaration.name(),
                            ));
                        }
                    }
                    (None, by_name)
                }
            };
            by_family.insert(
                InherentTypeFamily::Nominal(nominal),
                ConstructionSurface {
                    declaration: None,
                    structural,
                    variants,
                    functions: BTreeMap::new(),
                    literals: BTreeMap::new(),
                },
            );
        }

        for (construction, declaration) in graph.declarations().constructions().iter() {
            let family = InherentTypeFamily::of(types, declaration.target()).ok_or(
                ConstructionSurfaceBuildError::InvalidTarget(declaration.target()),
            )?;
            by_construction.insert(construction, family);
            match by_family.get_mut(&family) {
                Some(surface) => {
                    if surface.declaration.replace(construction).is_some() {
                        return Err(ConstructionSurfaceBuildError::DuplicateTarget(
                            declaration.target(),
                        ));
                    }
                }
                None => {
                    by_family.insert(family, ConstructionSurface::builtin(construction));
                }
            }
            let surface =
                by_family
                    .get_mut(&family)
                    .ok_or(ConstructionSurfaceBuildError::InvalidTarget(
                        declaration.target(),
                    ))?;
            index_construction_members(graph, construction, declaration.members(), surface)?;
        }
        Ok(Self {
            by_family,
            by_construction,
        })
    }

    #[must_use]
    pub fn for_nominal(&self, nominal: NominalTypeId) -> Option<ConstructionId> {
        self.by_family
            .get(&InherentTypeFamily::Nominal(nominal))
            .and_then(|surface| surface.declaration)
    }

    pub(crate) fn for_type(&self, types: &TypeStore, ty: TypeId) -> Option<ConstructionId> {
        InherentTypeFamily::of(types, ty)
            .and_then(|family| self.by_family.get(&family))
            .and_then(|surface| surface.declaration)
    }

    /// Selects the public construction surface presented at one module.
    ///
    /// The result preserves declaration order. Structural construction is first, followed by
    /// intrinsic variants and authored construct members. The default is an explicit fact rather
    /// than an ordering convention, so hover and completion can choose their own layout without
    /// reconstructing the surface.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the prepared table and declaration graph disagree.
    pub fn public_surface(
        &self,
        graph: &DeclarationGraph,
        nominal: NominalTypeId,
        from: ModuleId,
    ) -> Result<SelectedConstructionSurface, ConstructionSurfaceSelectionError> {
        self.select_surface(graph, nominal, from, SurfaceAudience::PublicPresentation)
    }

    /// Selects every construction entry accessible to source in one exact module.
    ///
    /// Unlike [`Self::public_surface`], this includes module-private structural construction and
    /// intrinsic variants when ordinary language visibility permits their use. Completion and
    /// other use-site tools must consume this view rather than the public documentation view.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the prepared table and declaration graph disagree.
    pub fn accessible_surface(
        &self,
        graph: &DeclarationGraph,
        nominal: NominalTypeId,
        from: ModuleId,
    ) -> Result<SelectedConstructionSurface, ConstructionSurfaceSelectionError> {
        self.select_surface(graph, nominal, from, SurfaceAudience::UseSite)
    }

    /// Selects the use-site construction surface attached to one built-in type family.
    ///
    /// Built-ins have no structural or intrinsic-variant entry. Their named functions and typed
    /// literals still pass through the same construction declaration, source order, default, and
    /// visibility authority as nominal types.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the prepared table and declaration graph disagree.
    pub(crate) fn accessible_builtin_surface(
        &self,
        graph: &DeclarationGraph,
        builtin: BuiltinType,
        from: ModuleId,
    ) -> Result<SelectedConstructionSurface, ConstructionSurfaceSelectionError> {
        let Some(surface) = self.by_family.get(&InherentTypeFamily::Builtin(builtin)) else {
            return Ok(SelectedConstructionSurface {
                declaration: None,
                entries: Box::new([]),
                default: None,
            });
        };
        let construction = surface
            .declaration
            .ok_or(ConstructionSurfaceSelectionError::MissingBuiltinConstruction(builtin))?;
        let (entries, default) =
            select_authored_entries(graph, surface, construction, from, SurfaceAudience::UseSite)?;
        Ok(SelectedConstructionSurface {
            declaration: Some(construction),
            entries: entries.into_boxed_slice(),
            default,
        })
    }

    fn select_surface(
        &self,
        graph: &DeclarationGraph,
        nominal: NominalTypeId,
        from: ModuleId,
        audience: SurfaceAudience,
    ) -> Result<SelectedConstructionSurface, ConstructionSurfaceSelectionError> {
        let surface = self
            .by_family
            .get(&InherentTypeFamily::Nominal(nominal))
            .ok_or(ConstructionSurfaceSelectionError::MissingNominal(nominal))?;
        let nominal_declaration = graph
            .declarations()
            .nominal_types()
            .get(nominal)
            .ok_or(ConstructionSurfaceSelectionError::MissingNominal(nominal))?;
        let construction = surface
            .declaration
            .map(|id| {
                graph
                    .declarations()
                    .constructions()
                    .get(id)
                    .map(|declaration| (id, declaration))
                    .ok_or(ConstructionSurfaceSelectionError::MissingConstruction(id))
            })
            .transpose()?;
        let structural_enabled = construction.is_none_or(|(_, declaration)| {
            declaration.default_member().is_none() && !declaration.members().is_empty()
        });
        let mut entries = Vec::new();
        let mut default = None;

        let structural_accessible = match audience {
            SurfaceAudience::PublicPresentation => structural_enabled,
            SurfaceAudience::UseSite => self.structural_fields(graph, nominal, from)?.is_some(),
        };
        if structural_accessible && let Some(structural) = surface.structural.as_ref() {
            let mut visible = true;
            for field in structural.fields.iter().copied() {
                visible &= field_is_visible(graph, nominal, field, from, audience)?;
            }
            if visible {
                if structural_enabled {
                    default = Some(entries.len());
                }
                entries.push(SelectedConstructionEntry::Structural);
            }
        }

        if let NominalShape::Enum { variants } = nominal_declaration.shape() {
            for variant in variants.iter().copied() {
                let declaration = graph
                    .declarations()
                    .variants()
                    .get(variant)
                    .ok_or(ConstructionSurfaceSelectionError::MissingVariant(variant))?;
                if declaration.owner() != nominal
                    || surface.variants.get(&declaration.name()) != Some(&variant)
                {
                    return Err(ConstructionSurfaceSelectionError::InvalidVariant(variant));
                }
                if site_is_visible(graph, declaration.site(), from, audience).ok_or(
                    ConstructionSurfaceSelectionError::MissingVariantSite(variant),
                )? {
                    entries.push(SelectedConstructionEntry::Variant(variant));
                }
            }
        }

        if let Some((construction, declaration)) = construction {
            let offset = entries.len();
            let (authored, authored_default) =
                select_authored_entries(graph, surface, construction, from, audience)?;
            entries.extend(authored);
            if let Some(authored_default) = authored_default {
                debug_assert!(declaration.default_member().is_some());
                default = Some(offset + authored_default);
            }
        }

        Ok(SelectedConstructionSurface {
            declaration: construction.map(|(id, _)| id),
            entries: entries.into_boxed_slice(),
            default,
        })
    }

    /// Returns the declared field identities when structural construction is accessible here.
    ///
    /// Field-level visibility remains the field selector's responsibility. This query owns the
    /// independent construction-surface restriction introduced by `default` and empty `construct`
    /// declarations.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the surface and declaration graph disagree.
    pub fn structural_fields<'a>(
        &'a self,
        graph: &DeclarationGraph,
        nominal: NominalTypeId,
        from: ModuleId,
    ) -> Result<Option<&'a [FieldId]>, ConstructionSurfaceSelectionError> {
        let surface = self
            .by_family
            .get(&InherentTypeFamily::Nominal(nominal))
            .ok_or(ConstructionSurfaceSelectionError::MissingNominal(nominal))?;
        let Some(structural) = surface.structural.as_ref() else {
            return Ok(None);
        };
        let declaration = graph
            .declarations()
            .nominal_types()
            .get(nominal)
            .ok_or(ConstructionSurfaceSelectionError::MissingNominal(nominal))?;
        let site = graph.declaration_sites().get(declaration.site()).ok_or(
            ConstructionSurfaceSelectionError::MissingNominalSite(nominal),
        )?;
        if site.module() == from {
            return Ok(Some(&structural.fields));
        }
        if let Some(construction) = surface.declaration {
            let declaration = graph
                .declarations()
                .constructions()
                .get(construction)
                .ok_or(ConstructionSurfaceSelectionError::MissingConstruction(
                    construction,
                ))?;
            if declaration.default_member().is_some() || declaration.members().is_empty() {
                return Ok(None);
            }
        }
        Ok(Some(&structural.fields))
    }

    /// Selects one named field from the already-authorized structural entry.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the nominal surface is missing.
    pub fn structural_field(
        &self,
        nominal: NominalTypeId,
        name: Symbol,
    ) -> Result<Option<FieldId>, ConstructionSurfaceSelectionError> {
        let surface = self
            .by_family
            .get(&InherentTypeFamily::Nominal(nominal))
            .ok_or(ConstructionSurfaceSelectionError::MissingNominal(nominal))?;
        Ok(surface
            .structural
            .as_ref()
            .and_then(|structural| structural.by_name.get(&name).copied()))
    }

    /// Selects one intrinsic variant entry by semantic owner and interned name.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the nominal surface is missing.
    pub fn variant(
        &self,
        nominal: NominalTypeId,
        name: Symbol,
    ) -> Result<Option<VariantId>, ConstructionSurfaceSelectionError> {
        let surface = self
            .by_family
            .get(&InherentTypeFamily::Nominal(nominal))
            .ok_or(ConstructionSurfaceSelectionError::MissingNominal(nominal))?;
        Ok(surface.variants.get(&name).copied())
    }

    /// Selects one accessible named construction function from an already resolved owner family.
    ///
    /// Member spelling, ownership, callable kind, and visibility are resolved here so body
    /// checking and editor consumers cannot grow separate construction-member lookup rules.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error if the validated declaration graph and surface index
    /// disagree or if multiple accessible members have the same semantic name.
    pub fn named_function(
        &self,
        graph: &DeclarationGraph,
        construction: ConstructionId,
        name: Symbol,
        from: ModuleId,
    ) -> Result<Option<CallableId>, ConstructionSurfaceSelectionError> {
        let surface = self.surface_for_construction(graph, construction)?;
        let Some(member) = surface.functions.get(&name).copied() else {
            return Ok(None);
        };
        Self::visible_member(graph, construction, member, from)
            .map(|visible| visible.then_some(member))
    }

    /// Selects the one accessible literal constructor for an exact language literal shape.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the immutable construction graph and its prepared
    /// surface disagree.
    pub fn literal(
        &self,
        graph: &DeclarationGraph,
        construction: ConstructionId,
        shape: LiteralShape,
        from: ModuleId,
    ) -> Result<Option<CallableId>, ConstructionSurfaceSelectionError> {
        let surface = self.surface_for_construction(graph, construction)?;
        let Some(member) = surface.literals.get(&shape).copied() else {
            return Ok(None);
        };
        Self::visible_member(graph, construction, member, from)
            .map(|visible| visible.then_some(member))
    }

    fn surface_for_construction(
        &self,
        graph: &DeclarationGraph,
        construction: ConstructionId,
    ) -> Result<&ConstructionSurface, ConstructionSurfaceSelectionError> {
        graph
            .declarations()
            .constructions()
            .get(construction)
            .ok_or(ConstructionSurfaceSelectionError::MissingConstruction(
                construction,
            ))?;
        let family = self.by_construction.get(&construction).ok_or(
            ConstructionSurfaceSelectionError::MissingConstruction(construction),
        )?;
        let surface = self.by_family.get(family).ok_or(
            ConstructionSurfaceSelectionError::MissingConstruction(construction),
        )?;
        if surface.declaration != Some(construction) {
            return Err(ConstructionSurfaceSelectionError::MissingConstruction(
                construction,
            ));
        }
        Ok(surface)
    }

    fn visible_member(
        graph: &DeclarationGraph,
        construction: ConstructionId,
        member: CallableId,
        from: ModuleId,
    ) -> Result<bool, ConstructionSurfaceSelectionError> {
        let callable = graph
            .declarations()
            .callables()
            .get(member)
            .ok_or(ConstructionSurfaceSelectionError::MissingCallable(member))?;
        if callable.owner() != CallableOwner::Construction(construction) {
            return Err(ConstructionSurfaceSelectionError::InvalidMember(member));
        }
        let site = graph
            .declaration_sites()
            .get(callable.site())
            .copied()
            .ok_or(ConstructionSurfaceSelectionError::MissingCallableSite(
                member,
            ))?;
        Ok(graph.is_visible_from(site.visibility(), from, site.module()))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_family.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_family.is_empty()
    }
}

#[derive(Clone, Copy)]
enum SurfaceAudience {
    UseSite,
    PublicPresentation,
}

fn select_authored_entries(
    graph: &DeclarationGraph,
    surface: &ConstructionSurface,
    construction: ConstructionId,
    from: ModuleId,
    audience: SurfaceAudience,
) -> Result<(Vec<SelectedConstructionEntry>, Option<usize>), ConstructionSurfaceSelectionError> {
    let declaration = graph
        .declarations()
        .constructions()
        .get(construction)
        .ok_or(ConstructionSurfaceSelectionError::MissingConstruction(
            construction,
        ))?;
    let mut entries = Vec::new();
    let mut default = None;
    for member in declaration.members().iter().copied() {
        if !surface_member_is_indexed(graph, surface, construction, member)? {
            return Err(ConstructionSurfaceSelectionError::InvalidMember(member));
        }
        if member_is_visible(graph, construction, member, from, audience)? {
            if declaration.default_member() == Some(member) {
                default = Some(entries.len());
            }
            entries.push(SelectedConstructionEntry::Callable(member));
        }
    }
    Ok((entries, default))
}

fn field_is_visible(
    graph: &DeclarationGraph,
    nominal: NominalTypeId,
    field: FieldId,
    from: ModuleId,
    audience: SurfaceAudience,
) -> Result<bool, ConstructionSurfaceSelectionError> {
    let declaration = graph
        .declarations()
        .fields()
        .get(field)
        .ok_or(ConstructionSurfaceSelectionError::MissingField(field))?;
    if declaration.owner() != nominal {
        return Err(ConstructionSurfaceSelectionError::InvalidField(field));
    }
    site_is_visible(graph, declaration.site(), from, audience)
        .ok_or(ConstructionSurfaceSelectionError::MissingFieldSite(field))
}

fn surface_member_is_indexed(
    graph: &DeclarationGraph,
    surface: &ConstructionSurface,
    construction: ConstructionId,
    member: CallableId,
) -> Result<bool, ConstructionSurfaceSelectionError> {
    let callable = graph
        .declarations()
        .callables()
        .get(member)
        .ok_or(ConstructionSurfaceSelectionError::MissingCallable(member))?;
    if callable.owner() != CallableOwner::Construction(construction) {
        return Err(ConstructionSurfaceSelectionError::InvalidMember(member));
    }
    Ok(match callable.kind() {
        CallableKind::ConstructionFunction => callable
            .name()
            .is_some_and(|name| surface.functions.get(&name) == Some(&member)),
        CallableKind::Literal(shape) => surface.literals.get(&shape) == Some(&member),
        _ => false,
    })
}

fn member_is_visible(
    graph: &DeclarationGraph,
    construction: ConstructionId,
    member: CallableId,
    from: ModuleId,
    audience: SurfaceAudience,
) -> Result<bool, ConstructionSurfaceSelectionError> {
    let callable = graph
        .declarations()
        .callables()
        .get(member)
        .ok_or(ConstructionSurfaceSelectionError::MissingCallable(member))?;
    if callable.owner() != CallableOwner::Construction(construction) {
        return Err(ConstructionSurfaceSelectionError::InvalidMember(member));
    }
    site_is_visible(graph, callable.site(), from, audience).ok_or(
        ConstructionSurfaceSelectionError::MissingCallableSite(member),
    )
}

fn site_is_visible(
    graph: &DeclarationGraph,
    site: nocter_model::DeclarationSiteId,
    from: ModuleId,
    audience: SurfaceAudience,
) -> Option<bool> {
    let site = graph.declaration_sites().get(site).copied()?;
    let visible = graph.is_visible_from(site.visibility(), from, site.module());
    Some(match audience {
        SurfaceAudience::UseSite => visible,
        SurfaceAudience::PublicPresentation => site.visibility() != Visibility::Private && visible,
    })
}

fn index_construction_members(
    graph: &DeclarationGraph,
    construction: ConstructionId,
    members: &[CallableId],
    surface: &mut ConstructionSurface,
) -> Result<(), ConstructionSurfaceBuildError> {
    for member in members.iter().copied() {
        let callable = graph
            .declarations()
            .callables()
            .get(member)
            .ok_or(ConstructionSurfaceBuildError::MissingCallable(member))?;
        if callable.owner() != CallableOwner::Construction(construction) {
            return Err(ConstructionSurfaceBuildError::InvalidMember(member));
        }
        match callable.kind() {
            CallableKind::ConstructionFunction => {
                let name = callable
                    .name()
                    .ok_or(ConstructionSurfaceBuildError::InvalidMember(member))?;
                if surface.functions.insert(name, member).is_some() {
                    return Err(ConstructionSurfaceBuildError::DuplicateFunction(
                        construction,
                        name,
                    ));
                }
            }
            CallableKind::Literal(shape) => {
                if surface.literals.insert(shape, member).is_some() {
                    return Err(ConstructionSurfaceBuildError::DuplicateLiteral(
                        construction,
                        shape,
                    ));
                }
            }
            _ => return Err(ConstructionSurfaceBuildError::InvalidMember(member)),
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionSurfaceBuildError {
    InvalidTarget(TypeId),
    DuplicateTarget(TypeId),
    MissingField(FieldId),
    InvalidFieldOwner(FieldId),
    DuplicateFieldName(NominalTypeId, Symbol),
    MissingVariant(VariantId),
    InvalidVariantOwner(VariantId),
    DuplicateVariantName(NominalTypeId, Symbol),
    MissingCallable(CallableId),
    InvalidMember(CallableId),
    DuplicateFunction(ConstructionId, Symbol),
    DuplicateLiteral(ConstructionId, LiteralShape),
}

impl std::fmt::Display for ConstructionSurfaceBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTarget(target) => {
                write!(formatter, "invalid construction target {target:?}")
            }
            Self::DuplicateTarget(target) => {
                write!(formatter, "duplicate construction target {target:?}")
            }
            Self::MissingField(field) => write!(formatter, "missing field {field:?}"),
            Self::InvalidFieldOwner(field) => {
                write!(formatter, "invalid owner for field {field:?}")
            }
            Self::DuplicateFieldName(nominal, name) => {
                write!(formatter, "duplicate field name {name:?} in {nominal:?}")
            }
            Self::MissingVariant(variant) => write!(formatter, "missing variant {variant:?}"),
            Self::InvalidVariantOwner(variant) => {
                write!(formatter, "invalid owner for variant {variant:?}")
            }
            Self::DuplicateVariantName(nominal, name) => {
                write!(formatter, "duplicate variant name {name:?} in {nominal:?}")
            }
            Self::MissingCallable(callable) => write!(formatter, "missing callable {callable:?}"),
            Self::InvalidMember(callable) => {
                write!(formatter, "invalid construction member {callable:?}")
            }
            Self::DuplicateFunction(construction, name) => write!(
                formatter,
                "duplicate construction function {name:?} in {construction:?}"
            ),
            Self::DuplicateLiteral(construction, shape) => {
                write!(formatter, "duplicate {shape:?} literal in {construction:?}")
            }
        }
    }
}

impl std::error::Error for ConstructionSurfaceBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionSurfaceSelectionError {
    MissingBuiltinConstruction(BuiltinType),
    MissingNominal(NominalTypeId),
    MissingNominalSite(NominalTypeId),
    MissingField(FieldId),
    MissingFieldSite(FieldId),
    InvalidField(FieldId),
    MissingVariant(VariantId),
    MissingVariantSite(VariantId),
    InvalidVariant(VariantId),
    MissingConstruction(ConstructionId),
    MissingCallable(CallableId),
    MissingCallableSite(CallableId),
    InvalidMember(CallableId),
    AmbiguousMember(ConstructionId, Symbol),
}

impl std::fmt::Display for ConstructionSurfaceSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid construction surface selection: {self:?}"
        )
    }
}

impl std::error::Error for ConstructionSurfaceSelectionError {}

#[cfg(test)]
mod tests {
    use nocter_declaration_lowering::lower_compile_unit_declarations;
    use nocter_declarations::ExportedEntity;

    use super::SelectedConstructionEntry;
    use crate::prepare_program_checking;
    use crate::test_support::Fixture;

    #[test]
    fn public_and_use_site_views_do_not_conflate_private_construction() {
        let fixture = Fixture::with_child(
            "use ./child\n",
            "struct Hidden {\n    value: i32\n}\n\nenum Choice {\n    only\n}\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let graph = prepared.graph();
        let child_name = graph.symbols().get("child").unwrap();
        let child = graph
            .modules()
            .iter()
            .find(|(_, module)| module.path().segments() == [child_name])
            .map(|(id, _)| id)
            .unwrap();
        let hidden_name = graph.symbols().get("Hidden").unwrap();
        let choice_name = graph.symbols().get("Choice").unwrap();
        let ExportedEntity::NominalType(hidden) = graph.lookup_local(child, hidden_name).unwrap()
        else {
            panic!("Hidden is not nominal");
        };
        let ExportedEntity::NominalType(choice) = graph.lookup_local(child, choice_name).unwrap()
        else {
            panic!("Choice is not nominal");
        };

        let surfaces = prepared.construction_surfaces();
        assert!(
            surfaces
                .public_surface(graph, hidden, child)
                .unwrap()
                .entries()
                .is_empty()
        );
        assert_eq!(
            surfaces
                .accessible_surface(graph, hidden, child)
                .unwrap()
                .entries(),
            &[SelectedConstructionEntry::Structural]
        );
        assert!(
            surfaces
                .public_surface(graph, choice, child)
                .unwrap()
                .entries()
                .is_empty()
        );
        assert!(matches!(
            surfaces
                .accessible_surface(graph, choice, child)
                .unwrap()
                .entries(),
            [SelectedConstructionEntry::Variant(_)]
        ));
    }
}
