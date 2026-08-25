use std::collections::BTreeMap;

use nocter_declarations::{
    CallableKind, CallableOwner, DeclarationGraph, LiteralShape, NominalShape,
};
use nocter_model::{
    AttachmentFamily, BuiltinType, CallableId, ConstructionId, FieldId, NominalTypeId, Symbol,
    TypeId, TypeStore, VariantId,
};

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
    by_family: BTreeMap<AttachmentFamily, ConstructionSurface>,
    by_construction: BTreeMap<ConstructionId, AttachmentFamily>,
}

impl ConstructionSurfaceTable {
    pub(crate) fn build_from_ids(
        graph: &DeclarationGraph,
        types: &TypeStore,
        constructions: &[ConstructionId],
    ) -> Result<Self, ConstructionSurfaceBuildError> {
        let mut by_family = nominal_surfaces(graph)?;
        let mut by_construction = BTreeMap::new();
        for construction in constructions {
            let construction = *construction;
            let declaration = graph
                .declarations()
                .constructions()
                .get(construction)
                .ok_or(ConstructionSurfaceBuildError::MissingConstruction(
                    construction,
                ))?;
            let family = AttachmentFamily::of(types, declaration.target()).ok_or(
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
    pub(crate) fn for_nominal(&self, nominal: NominalTypeId) -> Option<ConstructionId> {
        self.by_family
            .get(&AttachmentFamily::Nominal(nominal))
            .and_then(|surface| surface.declaration)
    }

    pub(crate) fn for_type(&self, types: &TypeStore, ty: TypeId) -> Option<ConstructionId> {
        AttachmentFamily::of(types, ty)
            .and_then(|family| self.by_family.get(&family))
            .and_then(|surface| surface.declaration)
    }

    /// Selects every construction entry accessible to one exact source and module.
    ///
    /// This includes source-private structural construction and intrinsic variants when
    /// direct-source access permits their use.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the prepared table and declaration graph disagree.
    pub(crate) fn accessible_surface(
        &self,
        graph: &DeclarationGraph,
        nominal: NominalTypeId,
        from: crate::SourceAccessContext<'_>,
    ) -> Result<Box<[SelectedConstructionEntry]>, ConstructionSurfaceSelectionError> {
        self.select_surface(graph, nominal, from)
    }

    /// Selects the use-site construction surface attached to one built-in type family.
    ///
    /// Built-ins have no structural or intrinsic-variant entry. Their named functions and typed
    /// literals still pass through the same construction declaration, source order, and visibility
    /// authority as nominal types.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the prepared table and declaration graph disagree.
    pub(crate) fn accessible_builtin_surface(
        &self,
        graph: &DeclarationGraph,
        builtin: BuiltinType,
        from: crate::SourceAccessContext<'_>,
    ) -> Result<Box<[SelectedConstructionEntry]>, ConstructionSurfaceSelectionError> {
        let Some(surface) = self.by_family.get(&AttachmentFamily::Builtin(builtin)) else {
            return Ok(Box::new([]));
        };
        let construction = surface
            .declaration
            .ok_or(ConstructionSurfaceSelectionError::MissingBuiltinConstruction(builtin))?;
        let entries = select_authored_entries(graph, surface, construction, from)?;
        Ok(entries.into_boxed_slice())
    }

    fn select_surface(
        &self,
        graph: &DeclarationGraph,
        nominal: NominalTypeId,
        from: crate::SourceAccessContext<'_>,
    ) -> Result<Box<[SelectedConstructionEntry]>, ConstructionSurfaceSelectionError> {
        let surface = self
            .by_family
            .get(&AttachmentFamily::Nominal(nominal))
            .ok_or(ConstructionSurfaceSelectionError::MissingNominal(nominal))?;
        let nominal_declaration = graph
            .declarations()
            .nominal_types()
            .get(nominal)
            .ok_or(ConstructionSurfaceSelectionError::MissingNominal(nominal))?;
        let construction = surface.declaration;
        if let Some(construction) = construction {
            graph
                .declarations()
                .constructions()
                .get(construction)
                .ok_or(ConstructionSurfaceSelectionError::MissingConstruction(
                    construction,
                ))?;
        }
        let mut entries = Vec::new();

        if self
            .accessible_structural_fields(graph, nominal, from)?
            .is_some()
        {
            entries.push(SelectedConstructionEntry::Structural);
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
                if from
                    .site_is_visible(graph, declaration.site())
                    .map_err(ConstructionSurfaceSelectionError::Visibility)?
                {
                    entries.push(SelectedConstructionEntry::Variant(variant));
                }
            }
        }

        if let Some(construction) = construction {
            let authored = select_authored_entries(graph, surface, construction, from)?;
            entries.extend(authored);
        }

        Ok(entries.into_boxed_slice())
    }

    /// Returns the declared field identities when the complete representation is accessible here.
    ///
    /// Field-level visibility remains the field selector's responsibility. This query owns only
    /// representation access; authored construction APIs do not affect structural construction.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the surface and declaration graph disagree.
    pub(crate) fn representation_fields<'a>(
        &'a self,
        nominal: NominalTypeId,
        from: crate::SourceAccessContext<'_>,
    ) -> Result<Option<&'a [FieldId]>, ConstructionSurfaceSelectionError> {
        let surface = self
            .by_family
            .get(&AttachmentFamily::Nominal(nominal))
            .ok_or(ConstructionSurfaceSelectionError::MissingNominal(nominal))?;
        let Some(structural) = surface.structural.as_ref() else {
            return Ok(None);
        };
        let representation_access = from
            .representation_is_visible(nominal)
            .map_err(ConstructionSurfaceSelectionError::Visibility)?;
        if !representation_access {
            return Ok(None);
        }
        Ok(Some(&structural.fields))
    }

    /// Returns all fields when structural construction is valid at this exact use site.
    ///
    /// Unlike [`Self::representation_fields`], this query owns the complete structural-entry
    /// decision: the representation and every field must be visible. Consumers that only need to
    /// know whether the structural entry exists should use this contract rather than reconstructing
    /// field visibility themselves.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the surface and declaration graph disagree.
    pub(crate) fn accessible_structural_fields<'a>(
        &'a self,
        graph: &DeclarationGraph,
        nominal: NominalTypeId,
        from: crate::SourceAccessContext<'_>,
    ) -> Result<Option<&'a [FieldId]>, ConstructionSurfaceSelectionError> {
        let Some(fields) = self.representation_fields(nominal, from)? else {
            return Ok(None);
        };
        for field in fields.iter().copied() {
            if !field_is_visible(graph, nominal, field, from)? {
                return Ok(None);
            }
        }
        Ok(Some(fields))
    }

    /// Selects one named field from the already-authorized structural entry.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error when the nominal surface is missing.
    pub(crate) fn structural_field(
        &self,
        nominal: NominalTypeId,
        name: Symbol,
    ) -> Result<Option<FieldId>, ConstructionSurfaceSelectionError> {
        let surface = self
            .by_family
            .get(&AttachmentFamily::Nominal(nominal))
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
    pub(crate) fn variant(
        &self,
        graph: &DeclarationGraph,
        nominal: NominalTypeId,
        name: Symbol,
        from: crate::SourceAccessContext<'_>,
    ) -> Result<Option<VariantId>, ConstructionSurfaceSelectionError> {
        let surface = self
            .by_family
            .get(&AttachmentFamily::Nominal(nominal))
            .ok_or(ConstructionSurfaceSelectionError::MissingNominal(nominal))?;
        let Some(variant) = surface.variants.get(&name).copied() else {
            return Ok(None);
        };
        let declaration = graph
            .declarations()
            .variants()
            .get(variant)
            .ok_or(ConstructionSurfaceSelectionError::MissingVariant(variant))?;
        if declaration.owner() != nominal {
            return Err(ConstructionSurfaceSelectionError::InvalidVariant(variant));
        }
        from.site_is_visible(graph, declaration.site())
            .map(|visible| visible.then_some(variant))
            .map_err(ConstructionSurfaceSelectionError::Visibility)
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
    pub(crate) fn named_function(
        &self,
        graph: &DeclarationGraph,
        construction: ConstructionId,
        name: Symbol,
        from: crate::SourceAccessContext<'_>,
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
    pub(crate) fn literal(
        &self,
        graph: &DeclarationGraph,
        construction: ConstructionId,
        shape: LiteralShape,
        from: crate::SourceAccessContext<'_>,
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
        from: crate::SourceAccessContext<'_>,
    ) -> Result<bool, ConstructionSurfaceSelectionError> {
        let callable = graph
            .declarations()
            .callables()
            .get(member)
            .ok_or(ConstructionSurfaceSelectionError::MissingCallable(member))?;
        if callable.owner() != CallableOwner::Construction(construction) {
            return Err(ConstructionSurfaceSelectionError::InvalidMember(member));
        }
        from.site_is_visible(graph, callable.site())
            .map_err(ConstructionSurfaceSelectionError::Visibility)
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.by_family.len()
    }
}

fn nominal_surfaces(
    graph: &DeclarationGraph,
) -> Result<BTreeMap<AttachmentFamily, ConstructionSurface>, ConstructionSurfaceBuildError> {
    let mut surfaces = BTreeMap::new();
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
                        return Err(ConstructionSurfaceBuildError::InvalidVariantOwner(variant));
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
        surfaces.insert(
            AttachmentFamily::Nominal(nominal),
            ConstructionSurface {
                declaration: None,
                structural,
                variants,
                functions: BTreeMap::new(),
                literals: BTreeMap::new(),
            },
        );
    }
    Ok(surfaces)
}

fn select_authored_entries(
    graph: &DeclarationGraph,
    surface: &ConstructionSurface,
    construction: ConstructionId,
    from: crate::SourceAccessContext<'_>,
) -> Result<Vec<SelectedConstructionEntry>, ConstructionSurfaceSelectionError> {
    let declaration = graph
        .declarations()
        .constructions()
        .get(construction)
        .ok_or(ConstructionSurfaceSelectionError::MissingConstruction(
            construction,
        ))?;
    let mut entries = Vec::new();
    for member in declaration.members().iter().copied() {
        if !surface_member_is_indexed(graph, surface, construction, member)? {
            return Err(ConstructionSurfaceSelectionError::InvalidMember(member));
        }
        if member_is_visible(graph, construction, member, from)? {
            entries.push(SelectedConstructionEntry::Callable(member));
        }
    }
    Ok(entries)
}

fn field_is_visible(
    graph: &DeclarationGraph,
    nominal: NominalTypeId,
    field: FieldId,
    from: crate::SourceAccessContext<'_>,
) -> Result<bool, ConstructionSurfaceSelectionError> {
    let declaration = graph
        .declarations()
        .fields()
        .get(field)
        .ok_or(ConstructionSurfaceSelectionError::MissingField(field))?;
    if declaration.owner() != nominal {
        return Err(ConstructionSurfaceSelectionError::InvalidField(field));
    }
    from.site_is_visible(graph, declaration.site())
        .map_err(ConstructionSurfaceSelectionError::Visibility)
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
    from: crate::SourceAccessContext<'_>,
) -> Result<bool, ConstructionSurfaceSelectionError> {
    let callable = graph
        .declarations()
        .callables()
        .get(member)
        .ok_or(ConstructionSurfaceSelectionError::MissingCallable(member))?;
    if callable.owner() != CallableOwner::Construction(construction) {
        return Err(ConstructionSurfaceSelectionError::InvalidMember(member));
    }
    from.site_is_visible(graph, callable.site())
        .map_err(ConstructionSurfaceSelectionError::Visibility)
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
    MissingConstruction(ConstructionId),
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
            Self::MissingConstruction(construction) => {
                write!(formatter, "missing construction {construction:?}")
            }
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
    MissingField(FieldId),
    InvalidField(FieldId),
    MissingVariant(VariantId),
    InvalidVariant(VariantId),
    MissingConstruction(ConstructionId),
    MissingCallable(CallableId),
    InvalidMember(CallableId),
    Visibility(crate::SourceVisibilityError),
    SourceAccess(nocter_frontend_bindings::SourceAccessError),
}

impl std::fmt::Display for ConstructionSurfaceSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid construction surface selection: {self:?}"
        )
    }
}

impl std::error::Error for ConstructionSurfaceSelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Visibility(error) => Some(error),
            Self::SourceAccess(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use nocter_declaration_lowering::lower_compile_unit_declarations;
    use nocter_declarations::ExportedEntity;

    use super::SelectedConstructionEntry;
    use crate::prepare_program_checking;
    use crate::test_support::Fixture;

    #[test]
    fn private_entries_are_available_in_their_owning_source() {
        let fixture = Fixture::with_child(
            "use ./child\n",
            "struct Hidden {\n    value: i32\n}\n\nenum Choice {\n    only\n}\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
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
        let child_source = frontend_bindings.module_sources(child).unwrap()[0];
        let child_access =
            crate::SourceAccessContext::for_source(prepared.source_access(), child_source).unwrap();
        assert_eq!(
            surfaces
                .accessible_surface(graph, hidden, child_access)
                .unwrap()
                .as_ref(),
            &[SelectedConstructionEntry::Structural]
        );
        assert!(matches!(
            surfaces
                .accessible_surface(graph, choice, child_access)
                .unwrap()
                .as_ref(),
            [SelectedConstructionEntry::Variant(_)]
        ));
    }

    #[test]
    fn opaque_enum_variants_remain_private_to_direct_source_access() {
        let fixture = Fixture::with_implementation_sources(
            concat!(
                "see ./choice.nct\n",
                "see ./consumer.nct\n",
                "\n",
                "pub enum Choice\n",
            ),
            &[
                ("choice.nct", "see ./index.nct\n\nenum Choice { hidden }\n"),
                ("consumer.nct", "see ./index.nct\n"),
            ],
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        let graph = prepared.graph();
        let choice_name = graph.symbols().get("Choice").unwrap();
        let module = graph
            .modules()
            .iter()
            .find(|(module, _)| graph.lookup_local(*module, choice_name).is_some())
            .map(|(module, _)| module)
            .unwrap();
        let ExportedEntity::NominalType(choice) = graph.lookup_local(module, choice_name).unwrap()
        else {
            panic!("Choice is not nominal");
        };
        let source_named = |suffix: &str| {
            frontend_bindings
                .module_sources(module)
                .unwrap()
                .iter()
                .copied()
                .find(|source| {
                    input
                        .sources()
                        .get(*source)
                        .unwrap()
                        .name()
                        .as_str()
                        .ends_with(suffix)
                })
                .unwrap()
        };
        let access = |suffix| {
            crate::SourceAccessContext::for_source(prepared.source_access(), source_named(suffix))
                .unwrap()
        };

        assert!(matches!(
            prepared
                .construction_surfaces()
                .accessible_surface(graph, choice, access("index.nct"))
                .unwrap()
                .as_ref(),
            [SelectedConstructionEntry::Variant(_)]
        ));
        assert!(
            prepared
                .construction_surfaces()
                .accessible_surface(graph, choice, access("consumer.nct"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn empty_opaque_struct_representation_requires_direct_source_access() {
        let fixture = Fixture::with_implementation_sources(
            concat!(
                "see ./empty.nct\n",
                "see ./consumer.nct\n",
                "\n",
                "pub struct Empty\n",
            ),
            &[
                ("empty.nct", "see ./index.nct\n\nstruct Empty {}\n"),
                ("consumer.nct", "see ./index.nct\n"),
            ],
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        let graph = prepared.graph();
        let empty_name = graph.symbols().get("Empty").unwrap();
        let module = graph
            .modules()
            .iter()
            .find(|(module, _)| graph.lookup_local(*module, empty_name).is_some())
            .map(|(module, _)| module)
            .unwrap();
        let ExportedEntity::NominalType(empty) = graph.lookup_local(module, empty_name).unwrap()
        else {
            panic!("Empty is not nominal");
        };
        let source_named = |suffix: &str| {
            frontend_bindings
                .module_sources(module)
                .unwrap()
                .iter()
                .copied()
                .find(|source| {
                    input
                        .sources()
                        .get(*source)
                        .unwrap()
                        .name()
                        .as_str()
                        .ends_with(suffix)
                })
                .unwrap()
        };
        let access = |suffix| {
            crate::SourceAccessContext::for_source(prepared.source_access(), source_named(suffix))
                .unwrap()
        };

        assert_eq!(
            prepared
                .construction_surfaces()
                .accessible_surface(graph, empty, access("index.nct"))
                .unwrap()
                .as_ref(),
            &[SelectedConstructionEntry::Structural]
        );
        assert!(
            prepared
                .construction_surfaces()
                .accessible_surface(graph, empty, access("consumer.nct"))
                .unwrap()
                .is_empty()
        );
    }
}
