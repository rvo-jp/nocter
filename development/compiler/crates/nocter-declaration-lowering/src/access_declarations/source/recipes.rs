//! Source-neutral capture/rebinding of executable roots and declaration header slots.
use super::{CodeHeaderSource, CodeSource, CodeSourceKind, FieldSource, RecordSource, Sources};
use crate::access_declarations::Failure;
use nocter_model::{AccessCodeId, NominalTypeId};
use nocter_syntax::{DeclarationSyntaxLocator, DeclarationSyntaxProjection, SyntaxOrigin};
use std::{collections::BTreeMap, sync::Arc};

/// Source-neutral recipe. Its locators have meaning only with an equal declaration surface.
#[derive(Clone, Debug)]
enum CodeRecipe {
    Declaration {
        header: CodeHeaderRecipe,
        body: Option<DeclarationSyntaxLocator>,
    },
    Expression(DeclarationSyntaxLocator),
}
#[derive(Clone, Debug)]
struct CodeHeaderRecipe {
    name: DeclarationSyntaxLocator,
    deferred_modifier: Option<DeclarationSyntaxLocator>,
    parameters: Box<[DeclarationSyntaxLocator]>,
    parameter_types: Box<[DeclarationSyntaxLocator]>,
    result: DeclarationSyntaxLocator,
}

/// Source-only record recipe; raw type normalization remains separate from rebinding.
#[derive(Clone, Debug)]
struct RecordRecipe {
    name: DeclarationSyntaxLocator,
    fields: Box<[(DeclarationSyntaxLocator, DeclarationSyntaxLocator)]>,
}

#[derive(Clone, Debug)]
pub(in crate::access_declarations) struct Recipes {
    codes: BTreeMap<AccessCodeId, CodeRecipe>,
    records: BTreeMap<NominalTypeId, RecordRecipe>,
}
impl Recipes {
    pub fn capture(
        projection: &DeclarationSyntaxProjection,
        sources: &Sources,
    ) -> Result<Self, Failure> {
        let locate = |origin| projection.locate(origin).ok_or(Failure::InconsistentSyntax);
        let capture_header = |header: &CodeHeaderSource| -> Result<CodeHeaderRecipe, Failure> {
            Ok(CodeHeaderRecipe {
                name: locate(SyntaxOrigin::Token(header.name))?,
                deferred_modifier: header
                    .deferred_modifier
                    .map(SyntaxOrigin::Node)
                    .map(locate)
                    .transpose()?,
                parameters: header
                    .parameters
                    .iter()
                    .map(|&name| locate(SyntaxOrigin::Token(name)))
                    .collect::<Result<_, _>>()?,
                parameter_types: header
                    .parameter_types
                    .iter()
                    .map(|&ty| locate(SyntaxOrigin::Node(ty)))
                    .collect::<Result<_, _>>()?,
                result: locate(SyntaxOrigin::Node(header.result))?,
            })
        };
        let mut recipes = BTreeMap::new();
        for (&id, source) in sources.codes.iter() {
            let recipe = match &source.kind {
                CodeSourceKind::Declaration { header, body } => CodeRecipe::Declaration {
                    header: capture_header(header)?,
                    body: body
                        .map(|body| locate(SyntaxOrigin::Node(body)))
                        .transpose()?,
                },
                CodeSourceKind::Expression(body) => {
                    CodeRecipe::Expression(locate(SyntaxOrigin::Node(*body))?)
                }
            };
            recipes.insert(id, recipe);
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
            codes: recipes,
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
        let bind_header = |header: &CodeHeaderRecipe| -> Result<CodeHeaderSource, Failure> {
            Ok(CodeHeaderSource {
                name: token(header.name)?,
                deferred_modifier: header.deferred_modifier.map(node).transpose()?,
                parameters: header
                    .parameters
                    .iter()
                    .map(|&locator| token(locator))
                    .collect::<Result<_, _>>()?,
                parameter_types: header
                    .parameter_types
                    .iter()
                    .map(|&locator| node(locator))
                    .collect::<Result<_, _>>()?,
                result: node(header.result)?,
            })
        };
        let sources = self
            .codes
            .iter()
            .map(|(&id, recipe)| {
                let source = match recipe {
                    CodeRecipe::Declaration { header, body } => {
                        CodeSource::declaration(bind_header(header)?, body.map(node).transpose()?)
                    }
                    CodeRecipe::Expression(body) => CodeSource::expression(node(*body)?),
                };
                Ok((id, source))
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
        Sources::new(sources, record_sources)
    }
}
