use std::fmt;

use nocter_declarations::{CallableKind, CallableOwner, DeclarationGraph};
use nocter_model::{
    BuiltinType, CallableId, ModuleId, NominalTypeId, Symbol, TypeKind, TypeStore, VariantId,
};

use crate::{
    CheckedProgram, ConstructionSurfaceSelectionError, ConstructionSurfaceTable,
    PreparedSemanticProgram, SelectedConstructionEntry,
};

/// The type-family identity to the left of one construction-member selection.
///
/// This identity deliberately excludes generic arguments. Named construction entries belong to a
/// type family, while argument inference remains the call checker's responsibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionCompletionOwner {
    Nominal(NominalTypeId),
    Builtin(BuiltinType),
}

/// One named construction entry available at an exact use site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConstructionCompletionCandidate {
    name: Symbol,
    target: ConstructionCompletionTarget,
}

impl ConstructionCompletionCandidate {
    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn target(self) -> ConstructionCompletionTarget {
        self.target
    }
}

/// The canonical semantic identity represented by a named construction completion item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionCompletionTarget {
    Variant(VariantId),
    Function(CallableId),
}

/// Failure to derive named construction completion from immutable compiler authorities.
#[derive(Debug)]
pub enum ConstructionCompletionError {
    Surface(ConstructionSurfaceSelectionError),
    MissingVariant(VariantId),
    InvalidVariant(VariantId),
    MissingCallable(CallableId),
    InvalidCallable(CallableId),
    MissingConstruction(nocter_model::ConstructionId),
    InvalidConstruction(nocter_model::ConstructionId),
    UnknownType(nocter_model::TypeId),
}

impl fmt::Display for ConstructionCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Surface(error) => error.fmt(formatter),
            Self::MissingVariant(variant) => {
                write!(
                    formatter,
                    "construction completion variant {variant:?} is absent"
                )
            }
            Self::InvalidVariant(variant) => {
                write!(
                    formatter,
                    "construction completion variant {variant:?} has another owner"
                )
            }
            Self::MissingCallable(callable) => {
                write!(
                    formatter,
                    "construction completion callable {callable:?} is absent"
                )
            }
            Self::InvalidCallable(callable) => {
                write!(
                    formatter,
                    "construction completion callable {callable:?} is not a named construction function"
                )
            }
            Self::MissingConstruction(construction) => {
                write!(
                    formatter,
                    "construction completion declaration {construction:?} is absent"
                )
            }
            Self::InvalidConstruction(construction) => {
                write!(
                    formatter,
                    "construction completion declaration {construction:?} has an invalid target"
                )
            }
            Self::UnknownType(ty) => {
                write!(formatter, "construction completion type {ty:?} is absent")
            }
        }
    }
}

impl std::error::Error for ConstructionCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Surface(error) => Some(error),
            Self::MissingVariant(_)
            | Self::InvalidVariant(_)
            | Self::MissingCallable(_)
            | Self::InvalidCallable(_)
            | Self::MissingConstruction(_)
            | Self::InvalidConstruction(_)
            | Self::UnknownType(_) => None,
        }
    }
}

impl From<ConstructionSurfaceSelectionError> for ConstructionCompletionError {
    fn from(error: ConstructionSurfaceSelectionError) -> Self {
        Self::Surface(error)
    }
}

impl CheckedProgram {
    /// Enumerates named construction entries through the ordinary use-site visibility view.
    ///
    /// # Errors
    ///
    /// Returns an error when the immutable declaration and construction authorities disagree.
    pub fn construction_completions(
        &self,
        owner: ConstructionCompletionOwner,
        module: ModuleId,
    ) -> Result<Box<[ConstructionCompletionCandidate]>, ConstructionCompletionError> {
        select_construction_completions(
            self.graph(),
            self.types(),
            self.construction_surfaces(),
            owner,
            module,
        )
    }

    /// Recovers the construction owner of an already selected semantic entry.
    ///
    /// # Errors
    ///
    /// Returns an error when the selected entry no longer agrees with its declaration authority.
    pub fn construction_completion_owner(
        &self,
        target: ConstructionCompletionTarget,
    ) -> Result<ConstructionCompletionOwner, ConstructionCompletionError> {
        construction_completion_owner(self.graph(), self.types(), target)
    }
}

impl PreparedSemanticProgram {
    /// Enumerates named construction entries from the completed pre-body semantic authority.
    ///
    /// # Errors
    ///
    /// Returns an error when the immutable declaration and construction authorities disagree.
    pub fn construction_completions(
        &self,
        owner: ConstructionCompletionOwner,
        module: ModuleId,
    ) -> Result<Box<[ConstructionCompletionCandidate]>, ConstructionCompletionError> {
        select_construction_completions(
            self.graph(),
            self.types(),
            self.construction_surfaces(),
            owner,
            module,
        )
    }
}

fn select_construction_completions(
    graph: &DeclarationGraph,
    types: &TypeStore,
    surfaces: &ConstructionSurfaceTable,
    owner: ConstructionCompletionOwner,
    module: ModuleId,
) -> Result<Box<[ConstructionCompletionCandidate]>, ConstructionCompletionError> {
    let surface = match owner {
        ConstructionCompletionOwner::Nominal(nominal) => {
            surfaces.accessible_surface(graph, nominal, module)?
        }
        ConstructionCompletionOwner::Builtin(builtin) => {
            surfaces.accessible_builtin_surface(graph, builtin, module)?
        }
    };
    let mut candidates = Vec::new();
    for entry in surface.entries().iter().copied() {
        match entry {
            SelectedConstructionEntry::Structural => {}
            SelectedConstructionEntry::Variant(variant) => {
                let declaration = graph
                    .declarations()
                    .variants()
                    .get(variant)
                    .ok_or(ConstructionCompletionError::MissingVariant(variant))?;
                if owner != ConstructionCompletionOwner::Nominal(declaration.owner()) {
                    return Err(ConstructionCompletionError::InvalidVariant(variant));
                }
                candidates.push(ConstructionCompletionCandidate {
                    name: declaration.name(),
                    target: ConstructionCompletionTarget::Variant(variant),
                });
            }
            SelectedConstructionEntry::Callable(callable) => {
                let declaration = graph
                    .declarations()
                    .callables()
                    .get(callable)
                    .ok_or(ConstructionCompletionError::MissingCallable(callable))?;
                match declaration.kind() {
                    CallableKind::ConstructionFunction => {
                        let name = declaration
                            .name()
                            .ok_or(ConstructionCompletionError::InvalidCallable(callable))?;
                        let actual = construction_completion_owner(
                            graph,
                            types,
                            ConstructionCompletionTarget::Function(callable),
                        )?;
                        if actual != owner {
                            return Err(ConstructionCompletionError::InvalidCallable(callable));
                        }
                        candidates.push(ConstructionCompletionCandidate {
                            name,
                            target: ConstructionCompletionTarget::Function(callable),
                        });
                    }
                    CallableKind::Literal(_) => {}
                    _ => return Err(ConstructionCompletionError::InvalidCallable(callable)),
                }
            }
        }
    }
    Ok(candidates.into_boxed_slice())
}

fn construction_completion_owner(
    graph: &DeclarationGraph,
    types: &TypeStore,
    target: ConstructionCompletionTarget,
) -> Result<ConstructionCompletionOwner, ConstructionCompletionError> {
    match target {
        ConstructionCompletionTarget::Variant(variant) => graph
            .declarations()
            .variants()
            .get(variant)
            .map(|declaration| ConstructionCompletionOwner::Nominal(declaration.owner()))
            .ok_or(ConstructionCompletionError::MissingVariant(variant)),
        ConstructionCompletionTarget::Function(callable) => {
            let callable_declaration = graph
                .declarations()
                .callables()
                .get(callable)
                .ok_or(ConstructionCompletionError::MissingCallable(callable))?;
            let CallableOwner::Construction(construction) = callable_declaration.owner() else {
                return Err(ConstructionCompletionError::InvalidCallable(callable));
            };
            if callable_declaration.kind() != CallableKind::ConstructionFunction
                || callable_declaration.name().is_none()
            {
                return Err(ConstructionCompletionError::InvalidCallable(callable));
            }
            let construction_declaration = graph
                .declarations()
                .constructions()
                .get(construction)
                .ok_or(
                ConstructionCompletionError::MissingConstruction(construction),
            )?;
            match types.get(construction_declaration.target()) {
                Some(TypeKind::Nominal { definition, .. }) => {
                    Ok(ConstructionCompletionOwner::Nominal(*definition))
                }
                Some(TypeKind::Builtin(builtin)) => {
                    Ok(ConstructionCompletionOwner::Builtin(*builtin))
                }
                Some(_) => Err(ConstructionCompletionError::InvalidConstruction(
                    construction,
                )),
                None => Err(ConstructionCompletionError::UnknownType(
                    construction_declaration.target(),
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use nocter_declaration_lowering::lower_compile_unit_declarations;
    use nocter_declarations::ExportedEntity;

    use super::{
        ConstructionCompletionOwner, ConstructionCompletionTarget, select_construction_completions,
    };
    use crate::prepare_program_checking;
    use crate::test_support::Fixture;

    #[test]
    fn completion_uses_the_owner_module_visibility_boundary_and_surface_order() {
        let fixture = Fixture::with_child(
            "use ./child\n",
            concat!(
                "pub enum Choice {\n",
                "    first\n",
                "    second(value: i32)\n",
                "}\n",
                "construct Choice {\n",
                "    pub func public_new(): Self { loop {} }\n",
                "    pub(./) func local_new(): Self { loop {} }\n",
                "    pub literal [](...items: i32): Self { loop {} }\n",
                "}\n",
            ),
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
        let package = graph.modules().get(child).unwrap().package();
        let root = graph
            .modules()
            .iter()
            .find(|(_, module)| module.package() == package && module.path().segments().is_empty())
            .map(|(id, _)| id)
            .unwrap();
        let choice_name = graph.symbols().get("Choice").unwrap();
        let ExportedEntity::NominalType(choice) = graph.lookup_local(child, choice_name).unwrap()
        else {
            panic!("Choice is not nominal");
        };

        let names = |module| {
            select_construction_completions(
                graph,
                prepared.types(),
                prepared.construction_surfaces(),
                ConstructionCompletionOwner::Nominal(choice),
                module,
            )
            .unwrap()
            .iter()
            .map(|candidate| {
                (
                    graph.symbols().spelling(candidate.name()).unwrap(),
                    candidate.target(),
                )
            })
            .collect::<Vec<_>>()
        };
        let local = names(child);
        assert_eq!(
            local.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            ["first", "second", "public_new", "local_new"]
        );
        assert!(matches!(
            local.as_slice(),
            [
                (_, ConstructionCompletionTarget::Variant(_)),
                (_, ConstructionCompletionTarget::Variant(_)),
                (_, ConstructionCompletionTarget::Function(_)),
                (_, ConstructionCompletionTarget::Function(_)),
            ]
        ));
        assert_eq!(
            names(root)
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>(),
            ["first", "second", "public_new"]
        );
    }
}
