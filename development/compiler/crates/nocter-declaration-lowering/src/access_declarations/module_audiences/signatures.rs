//! Public semantic type names, never hidden representation payloads or const supplier names.
use super::{ModuleAudience, ModuleAudiences, VisibilityDiagnostic, VisibilityRule};
use crate::access_declarations::{Failure, module_scopes::ParsedHeader};
use crate::{DeclarationSurface, SurfaceDeclarationId, SurfaceDeclarationKind};
use nocter_declarations::value_access::{
    AggregateForm, DeclarationId, Namespace, PayloadSchema, Program, QualifiedNameError, ScopeId,
};
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::{AccessFieldId, BuiltinType, TypeId};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{BoundSyntax, NodeId, NodeKind, SyntaxOrigin, direct_node};
use std::collections::{BTreeMap, BTreeSet, HashMap};

impl ModuleAudiences {
    pub fn verify_signatures(
        &self,
        surface: &DeclarationSurface<'_>,
        declarations: &[Option<DeclarationId>],
        fields: &[Option<AccessFieldId>],
        program: &Program,
        headers: &BTreeMap<SurfaceDeclarationId, ParsedHeader>,
        scopes: &[ScopeId],
    ) -> Result<(), Failure> {
        let exported_types = surface
            .declarations()
            .iter()
            .enumerate()
            .filter_map(|(index, _)| {
                let raw = SurfaceDeclarationId::from_index(index);
                let audience = self.declaration(raw)?;
                let declaration @ (DeclarationId::Record(_) | DeclarationId::Alias(_)) =
                    declarations[index]?
                else {
                    return None;
                };
                Some((declaration, (raw, audience)))
            })
            .collect::<HashMap<_, _>>();
        let checking = Signatures {
            surface,
            declarations,
            program,
            exported_types,
        };
        for (index, declaration) in surface.declarations().iter().enumerate() {
            let raw = SurfaceDeclarationId::from_index(index);
            let Some(audience) = self.declaration(raw) else {
                continue;
            };
            let tree = surface.sources()[declaration.source().index()].syntax();
            let syntax = BoundSyntax::new(
                surface
                    .source_map()
                    .get(tree.source())
                    .ok_or(Failure::InconsistentSyntax)?,
                tree,
            )
            .ok_or(Failure::InconsistentSyntax)?;
            match declarations[index] {
                Some(DeclarationId::Code(id)) => {
                    let Some(ParsedHeader::Code(header)) = headers.get(&raw) else {
                        return Err(Failure::InconsistentSyntax);
                    };
                    let code = program.codes().get(id).ok_or(Failure::InconsistentSyntax)?;
                    if code.parameters().len() != header.parameters.len() {
                        return Err(Failure::InconsistentSyntax);
                    }
                    for (&parameter, source) in code.parameters().iter().zip(&header.parameters) {
                        let parameter = program
                            .parameters()
                            .get(parameter)
                            .ok_or(Failure::InconsistentSyntax)?;
                        checking.check(
                            audience,
                            syntax,
                            scopes[index],
                            source.ty,
                            parameter.data(),
                        )?;
                    }
                    checking.check(
                        audience,
                        syntax,
                        scopes[index],
                        header.result_syntax,
                        code.result(),
                    )?;
                }
                None if declaration.kind() == SurfaceDeclarationKind::Field => {
                    let id = fields[index].ok_or(Failure::InconsistentSyntax)?;
                    let field = program
                        .fields()
                        .get(id)
                        .ok_or(Failure::InconsistentSyntax)?;
                    let ty = direct_node(tree, declaration.node(), NodeKind::Type)
                        .ok_or(Failure::InconsistentSyntax)?;
                    checking.check(audience, syntax, scopes[index], ty, field.data())?;
                }
                Some(DeclarationId::Record(_)) => {}
                Some(DeclarationId::Alias(id)) => {
                    let Some(ParsedHeader::Alias(header)) = headers.get(&raw) else {
                        return Err(Failure::InconsistentSyntax);
                    };
                    let alias = program.alias(id).ok_or(Failure::InconsistentSyntax)?;
                    checking.check(
                        audience,
                        syntax,
                        scopes[index],
                        header.source.ty,
                        alias.data(),
                    )?;
                }
                None if matches!(
                    declaration.kind(),
                    SurfaceDeclarationKind::TypeBehavior
                        | SurfaceDeclarationKind::StructuralLiteralGrant
                        | SurfaceDeclarationKind::PrimitiveType
                ) => {}
                _ => return Err(Failure::InconsistentSyntax),
            }
        }
        Ok(())
    }
}
struct Signatures<'a, 'syntax> {
    surface: &'a DeclarationSurface<'syntax>,
    declarations: &'a [Option<DeclarationId>],
    program: &'a Program,
    exported_types: HashMap<DeclarationId, (SurfaceDeclarationId, &'a ModuleAudience)>,
}
impl Signatures<'_, '_> {
    fn check(
        &self,
        audience: &ModuleAudience,
        syntax: BoundSyntax<'_>,
        scope: ScopeId,
        primary: NodeId,
        data: TypeId,
    ) -> Result<(), Failure> {
        self.check_type_paths(audience, syntax, scope, primary)?;
        let mut seen = BTreeSet::new();
        let mut pending = vec![data];
        while let Some(data) = pending.pop() {
            if !seen.insert(data) {
                continue;
            }
            if let Some(record) = self.program.record_identity(data) {
                if !self.visible_to(audience, record.into()) {
                    return self.reject(syntax, SyntaxOrigin::Node(primary), record.into());
                }
                // Representation fields do not become part of the outward nominal contract.
                continue;
            }
            match self
                .program
                .data_schema(data)
                .ok_or(Failure::InconsistentSyntax)?
            {
                PayloadSchema::Builtin(_) => {}
                PayloadSchema::Optional(payload)
                | PayloadSchema::Fallible(payload)
                | PayloadSchema::Future(payload) => {
                    pending.push(payload);
                }
                PayloadSchema::Array(array) => pending.push(array.element()),
                PayloadSchema::Aggregate(aggregate) => {
                    let AggregateForm::Tuple(fields) = aggregate.form() else {
                        return Err(Failure::InconsistentSyntax);
                    };
                    pending.extend(fields.iter().rev().copied());
                }
                PayloadSchema::Function(witness) => {
                    let contract = self
                        .program
                        .function_contract(witness)
                        .ok_or(Failure::InconsistentSyntax)?;
                    pending.push(contract.result);
                    pending.extend(contract.inputs.iter().rev().map(|input| input.data));
                }
            }
        }
        Ok(())
    }
    fn check_type_paths(
        &self,
        audience: &ModuleAudience,
        syntax: BoundSyntax<'_>,
        scope: ScopeId,
        root: NodeId,
    ) -> Result<(), Failure> {
        if syntax
            .tree()
            .node(root)
            .is_none_or(|node| node.kind() != NodeKind::Type)
        {
            return Ok(());
        }
        let source = crate::access_declarations::project_type_syntax(syntax, root)?;
        for path in source.named_paths() {
            if let [token] = path
                && syntax.text_at(token.range()).is_some_and(|name| {
                    name == "Self" || BuiltinType::from_spelling(name).is_some()
                })
            {
                continue;
            }
            let names = path
                .iter()
                .map(|name| {
                    syntax
                        .text_at(name.range())
                        .and_then(|name| self.program.symbols().get(name))
                })
                .collect::<Vec<_>>();
            match self.program.declaration_path_for_audience(
                scope,
                Namespace::Type,
                &names,
                audience,
            ) {
                Ok(alias @ DeclarationId::Alias(_)) if !self.visible_to(audience, alias) => {
                    let name = *path.last().ok_or(Failure::InconsistentSyntax)?;
                    return self.reject(syntax, SyntaxOrigin::Token(name), alias);
                }
                Ok(_) => {}
                Err(QualifiedNameError::Inaccessible { position }) => {
                    let name = *path.get(position).ok_or(Failure::InconsistentSyntax)?;
                    let primary = SourceOrigin::from_token(syntax.tree(), name)
                        .map_err(|_| Failure::InconsistentSyntax)?;
                    return Err(Failure::Visibility(Box::new(VisibilityDiagnostic {
                        rule: VisibilityRule::SignatureExposure,
                        source: SourceDiagnostic::new(
                            DiagnosticCode::E0244,
                            "this type path component has a narrower audience than its exported signature",
                            primary,
                            [],
                            None::<&str>,
                        ),
                    })));
                }
                Err(_) => return Err(Failure::InconsistentSyntax),
            }
        }
        Ok(())
    }
    fn visible_to(&self, audience: &ModuleAudience, target: DeclarationId) -> bool {
        self.exported_types
            .get(&target)
            .is_some_and(|(_, target)| audience.is_within(target))
    }
    fn reject(
        &self,
        syntax: BoundSyntax<'_>,
        primary: SyntaxOrigin,
        target: DeclarationId,
    ) -> Result<(), Failure> {
        let raw = match self.exported_types.get(&target) {
            Some((raw, _)) => *raw,
            None => self
                .declarations
                .iter()
                .position(|id| *id == Some(target))
                .map(SurfaceDeclarationId::from_index)
                .ok_or(Failure::InconsistentSyntax)?,
        };
        let declaration = self.surface.declarations()[raw.index()];
        let tree = self.surface.sources()[declaration.source().index()].syntax();
        let token = declaration.name().ok_or(Failure::InconsistentSyntax)?;
        let primary =
            crate::diagnostic::origin_from_syntax(std::iter::once(syntax.tree()), primary)
                .ok_or(Failure::InconsistentSyntax)?;
        let related =
            SourceOrigin::from_token(tree, token).map_err(|_| Failure::InconsistentSyntax)?;
        Err(Failure::Visibility(Box::new(VisibilityDiagnostic {
            rule: VisibilityRule::SignatureExposure,
            source: SourceDiagnostic::new(
                DiagnosticCode::E0244,
                "this exported signature exposes a type outside its effective audience",
                primary,
                [DiagnosticNote::new(
                    "this type has a narrower or source-private audience",
                    related,
                )],
                None::<&str>,
            ),
        })))
    }
}
