//! DefId-keyed projection from semantic declarations to resolver surfaces.

use super::{
    AssociatedFunctionSignature, AssociatedTypeSignature, CoercionSignature, DestructSignature,
    EnumVariantSignature, InterfaceConformance, LiteralSignature, MethodSignature, ResolveOutput,
    StructFieldSignature, Symbol, SymbolId, SymbolKind, TypeSymbol,
};
use crate::builtin_types::BuiltinTypeOwner;
use crate::semantic::DefId;
use crate::source::ByteSpan;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnerLocator {
    Symbol(SymbolId),
    Builtin(BuiltinTypeOwner),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclarationLocator {
    Symbol(SymbolId),
    Field(OwnerLocator, usize),
    Variant(OwnerLocator, usize),
    AssociatedType(OwnerLocator, usize),
    AssociatedFunction(OwnerLocator, usize),
    Method(OwnerLocator, usize),
    Conformance(OwnerLocator, usize),
    ConformanceMethod(OwnerLocator, usize, usize),
    Destructor(OwnerLocator),
    Literal(OwnerLocator, usize),
    Coercion(OwnerLocator, usize),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct DeclarationIndex {
    by_definition: HashMap<DefId, DeclarationLocator>,
}

impl DeclarationIndex {
    pub(super) fn build(output: &ResolveOutput) -> Self {
        let mut index = Self {
            by_definition: HashMap::new(),
        };
        for symbol in output.symbols.symbols() {
            let canonical = output
                .semantic_db
                .definition_anchor(symbol.def_id)
                .is_some_and(|anchor| anchor == symbol.name_span)
                || output
                    .semantic_db
                    .definition_span(symbol.def_id)
                    .is_some_and(|span| span == symbol.declaration_span);
            if canonical {
                index
                    .by_definition
                    .insert(symbol.def_id, DeclarationLocator::Symbol(symbol.id));
            } else {
                index
                    .by_definition
                    .entry(symbol.def_id)
                    .or_insert(DeclarationLocator::Symbol(symbol.id));
            }
            if let SymbolKind::Type(owner) = &symbol.kind {
                index.collect_owner(output, OwnerLocator::Symbol(symbol.id), owner);
            }
        }
        for surface in output.builtin_type_surfaces.values() {
            index.collect_owner(
                output,
                OwnerLocator::Builtin(surface.owner),
                &surface.symbol,
            );
        }
        index
    }

    fn collect_owner(&mut self, output: &ResolveOutput, locator: OwnerLocator, owner: &TypeSymbol) {
        self.collect_members(output, locator, &owner.fields, |owner, index| {
            DeclarationLocator::Field(owner, index)
        });
        self.collect_members(output, locator, &owner.variants, |owner, index| {
            DeclarationLocator::Variant(owner, index)
        });
        self.collect_members(output, locator, &owner.associated_types, |owner, index| {
            DeclarationLocator::AssociatedType(owner, index)
        });
        self.collect_members(
            output,
            locator,
            &owner.associated_functions,
            DeclarationLocator::AssociatedFunction,
        );
        self.collect_members(output, locator, &owner.methods, |owner, index| {
            DeclarationLocator::Method(owner, index)
        });
        for (conformance_index, conformance) in owner.interface_conformances.iter().enumerate() {
            self.insert(
                output,
                conformance.declaration_span,
                DeclarationLocator::Conformance(locator, conformance_index),
            );
            for (method_index, method) in conformance.methods.iter().enumerate() {
                self.insert(
                    output,
                    method.name_span,
                    DeclarationLocator::ConformanceMethod(locator, conformance_index, method_index),
                );
            }
        }
        if let Some(destructor) = &owner.destructor {
            self.insert(
                output,
                destructor.name_span,
                DeclarationLocator::Destructor(locator),
            );
        }
        self.collect_members(output, locator, &owner.literals, |owner, index| {
            DeclarationLocator::Literal(owner, index)
        });
        self.collect_members(output, locator, &owner.coercions, |owner, index| {
            DeclarationLocator::Coercion(owner, index)
        });
    }

    fn collect_members<T: DeclarationAnchor>(
        &mut self,
        output: &ResolveOutput,
        owner: OwnerLocator,
        members: &[T],
        locator: impl Fn(OwnerLocator, usize) -> DeclarationLocator,
    ) {
        for (index, member) in members.iter().enumerate() {
            self.insert(output, member.declaration_anchor(), locator(owner, index));
        }
    }

    fn insert(&mut self, output: &ResolveOutput, anchor: ByteSpan, locator: DeclarationLocator) {
        let Some(definition) = output.semantic_db.definition_at(anchor) else {
            return;
        };
        self.by_definition.entry(definition).or_insert(locator);
    }

    pub(super) fn get<'a>(
        &self,
        output: &'a ResolveOutput,
        definition: DefId,
    ) -> Option<ResolvedDeclaration<'a>> {
        let locator = *self.by_definition.get(&definition)?;
        Some(match locator {
            DeclarationLocator::Symbol(id) => ResolvedDeclaration::Symbol(output.symbols.get(id)?),
            DeclarationLocator::Field(owner, index) => {
                let owner = owner.get(output)?;
                ResolvedDeclaration::Field(owner, owner.fields.get(index)?)
            }
            DeclarationLocator::Variant(owner, index) => {
                let owner = owner.get(output)?;
                ResolvedDeclaration::Variant(owner, owner.variants.get(index)?)
            }
            DeclarationLocator::AssociatedType(owner, index) => {
                let owner = owner.get(output)?;
                ResolvedDeclaration::AssociatedType(owner, owner.associated_types.get(index)?)
            }
            DeclarationLocator::AssociatedFunction(owner, index) => {
                let owner = owner.get(output)?;
                ResolvedDeclaration::AssociatedFunction(
                    owner,
                    owner.associated_functions.get(index)?,
                )
            }
            DeclarationLocator::Method(owner, index) => {
                let owner = owner.get(output)?;
                ResolvedDeclaration::Method(owner, owner.methods.get(index)?)
            }
            DeclarationLocator::Conformance(owner, index) => {
                let owner = owner.get(output)?;
                ResolvedDeclaration::Conformance(owner.interface_conformances.get(index)?)
            }
            DeclarationLocator::ConformanceMethod(owner, conformance, method) => {
                let owner = owner.get(output)?;
                ResolvedDeclaration::Method(
                    owner,
                    owner
                        .interface_conformances
                        .get(conformance)?
                        .methods
                        .get(method)?,
                )
            }
            DeclarationLocator::Destructor(owner) => {
                let owner = owner.get(output)?;
                ResolvedDeclaration::Destructor(owner, owner.destructor.as_ref()?)
            }
            DeclarationLocator::Literal(owner, index) => {
                let owner = owner.get(output)?;
                ResolvedDeclaration::Literal(owner, owner.literals.get(index)?)
            }
            DeclarationLocator::Coercion(owner, index) => {
                let owner = owner.get(output)?;
                ResolvedDeclaration::Coercion(owner.coercions.get(index)?)
            }
        })
    }
}

impl OwnerLocator {
    fn get(self, output: &ResolveOutput) -> Option<&TypeSymbol> {
        match self {
            Self::Symbol(id) => match &output.symbols.get(id)?.kind {
                SymbolKind::Type(owner) => Some(owner),
                _ => None,
            },
            Self::Builtin(owner) => output
                .builtin_type_surfaces
                .get(&owner)
                .map(|surface| &surface.symbol),
        }
    }
}

trait DeclarationAnchor {
    fn declaration_anchor(&self) -> ByteSpan;
}

macro_rules! anchor {
    ($type:ty, $field:ident) => {
        impl DeclarationAnchor for $type {
            fn declaration_anchor(&self) -> ByteSpan {
                self.$field
            }
        }
    };
}

anchor!(StructFieldSignature, name_span);
anchor!(EnumVariantSignature, name_span);
anchor!(AssociatedTypeSignature, name_span);
anchor!(AssociatedFunctionSignature, name_span);
anchor!(MethodSignature, name_span);
anchor!(LiteralSignature, shape_span);
anchor!(CoercionSignature, focus_span);

pub(crate) enum ResolvedDeclaration<'a> {
    Symbol(&'a Symbol),
    Field(&'a TypeSymbol, &'a StructFieldSignature),
    Variant(&'a TypeSymbol, &'a EnumVariantSignature),
    AssociatedType(&'a TypeSymbol, &'a AssociatedTypeSignature),
    AssociatedFunction(&'a TypeSymbol, &'a AssociatedFunctionSignature),
    Method(&'a TypeSymbol, &'a MethodSignature),
    Conformance(&'a InterfaceConformance),
    Destructor(&'a TypeSymbol, &'a DestructSignature),
    Literal(&'a TypeSymbol, &'a LiteralSignature),
    Coercion(&'a CoercionSignature),
}

#[cfg(test)]
mod tests {
    use crate::lexer::lex;
    use crate::parser::parse;
    use crate::semantic::DefinitionKind;
    use crate::source::SourceMap;

    #[test]
    fn indexes_every_resolver_surface_by_semantic_definition() {
        let mut sources = SourceMap::new();
        let source = sources.add_source(
            "index.nct",
            None,
            r#"interface Named {
    pub type Item
    pub method &self.name(): &str
}
struct Record { pub value: i32 }
enum Choice { one(value: i32) }
instance Record {
    pub method &self.name(): &str { return "record" }
    pub coerce &self as &str { return self.name() }
}
construct Record {
    pub literal ""(text: &str): Self { return Record { value: 0 } }
}
conform Named for Record {
    type Item = i32
    method &self.name(): &str { return "record" }
}
"#,
        );
        let lexed = lex(&sources, source);
        assert!(lexed.diagnostics.is_empty(), "{:?}", lexed.diagnostics);
        let parsed = parse(&sources, source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let ast = parsed.ast.unwrap();
        let output = crate::resolve::resolve(&sources, &ast);

        for definition in output.semantic_db.definitions() {
            if matches!(
                definition.kind,
                DefinitionKind::StructField
                    | DefinitionKind::EnumVariant
                    | DefinitionKind::AssociatedType
                    | DefinitionKind::Method
                    | DefinitionKind::Conformance
                    | DefinitionKind::Literal
                    | DefinitionKind::Coercion
            ) {
                assert!(
                    output.declaration(definition.id).is_some(),
                    "missing resolver projection for {definition:?}"
                );
            }
        }
    }
}
