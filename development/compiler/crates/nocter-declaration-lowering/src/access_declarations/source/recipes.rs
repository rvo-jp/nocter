//! Source-neutral capture/rebinding of named bodies and declaration header slots.
use super::{CallableSource, FieldSource, RecordSource, Sources};
use crate::access_declarations::Failure;
use nocter_model::{AccessCallableId, NominalTypeId};
use nocter_syntax::{DeclarationSyntaxLocator, DeclarationSyntaxProjection, SyntaxOrigin};
use std::{collections::BTreeMap, sync::Arc};

/// Source-neutral recipe. Its locators have meaning only with an equal declaration surface.
#[derive(Clone, Debug)]
struct CallableRecipe {
    name: DeclarationSyntaxLocator,
    parameters: Box<[DeclarationSyntaxLocator]>,
    parameter_types: Box<[DeclarationSyntaxLocator]>,
    result: DeclarationSyntaxLocator,
    body: DeclarationSyntaxLocator,
}

/// Source-only record recipe; raw type normalization remains separate from rebinding.
#[derive(Clone, Debug)]
struct RecordRecipe {
    name: DeclarationSyntaxLocator,
    fields: Box<[(DeclarationSyntaxLocator, DeclarationSyntaxLocator)]>,
}

#[derive(Clone, Debug)]
pub(in crate::access_declarations) struct Recipes {
    callables: BTreeMap<AccessCallableId, CallableRecipe>,
    records: BTreeMap<NominalTypeId, RecordRecipe>,
}
impl Recipes {
    pub fn capture(
        projection: &DeclarationSyntaxProjection,
        sources: &Sources,
    ) -> Result<Self, Failure> {
        let locate = |origin| projection.locate(origin).ok_or(Failure::InconsistentSyntax);
        let mut recipes = BTreeMap::new();
        for (&id, source) in sources.callables.iter() {
            recipes.insert(
                id,
                CallableRecipe {
                    name: locate(SyntaxOrigin::Token(source.name))?,
                    parameters: source
                        .parameters
                        .iter()
                        .map(|&name| locate(SyntaxOrigin::Token(name)))
                        .collect::<Result<_, _>>()?,
                    parameter_types: source
                        .parameter_types
                        .iter()
                        .map(|&ty| locate(SyntaxOrigin::Node(ty)))
                        .collect::<Result<_, _>>()?,
                    result: locate(SyntaxOrigin::Node(source.result))?,
                    body: locate(SyntaxOrigin::Node(source.body))?,
                },
            );
        }
        let record_recipes = sources
            .records
            .iter()
            .map(|(&id, source)| {
                Ok((
                    id,
                    RecordRecipe {
                        name: locate(SyntaxOrigin::Token(source.name))?,
                        fields: source
                            .fields
                            .iter()
                            .map(|field| {
                                Ok((
                                    locate(SyntaxOrigin::Token(field.name))?,
                                    locate(SyntaxOrigin::Node(field.ty))?,
                                ))
                            })
                            .collect::<Result<_, Failure>>()?,
                    },
                ))
            })
            .collect::<Result<_, Failure>>()?;
        Ok(Self {
            callables: recipes,
            records: record_recipes,
        })
    }
    pub fn bind(&self, projection: &DeclarationSyntaxProjection) -> Result<Sources, Failure> {
        let token = |locator| match projection.resolve(locator) {
            Some(SyntaxOrigin::Token(token)) => Ok(token),
            _ => Err(Failure::InconsistentSyntax),
        };
        let node = |locator| match projection.resolve(locator) {
            Some(SyntaxOrigin::Node(node)) => Ok(node),
            _ => Err(Failure::InconsistentSyntax),
        };
        let sources = self
            .callables
            .iter()
            .map(|(&id, recipe)| {
                Ok((
                    id,
                    CallableSource {
                        name: token(recipe.name)?,
                        parameters: recipe
                            .parameters
                            .iter()
                            .map(|&locator| token(locator))
                            .collect::<Result<_, _>>()?,
                        parameter_types: recipe
                            .parameter_types
                            .iter()
                            .map(|&locator| node(locator))
                            .collect::<Result<_, _>>()?,
                        result: node(recipe.result)?,
                        body: node(recipe.body)?,
                    },
                ))
            })
            .collect::<Result<_, Failure>>()?;
        let record_sources = self
            .records
            .iter()
            .map(|(&id, recipe)| {
                Ok((
                    id,
                    Arc::new(RecordSource {
                        name: token(recipe.name)?,
                        fields: recipe
                            .fields
                            .iter()
                            .map(|&(name, ty)| {
                                Ok(FieldSource {
                                    name: token(name)?,
                                    ty: node(ty)?,
                                })
                            })
                            .collect::<Result<_, Failure>>()?,
                    }),
                ))
            })
            .collect::<Result<_, Failure>>()?;
        Ok(Sources {
            callables: Arc::new(sources),
            records: Arc::new(record_sources),
        })
    }
}
