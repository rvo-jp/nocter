//! Public semantic type names, never hidden representation payloads or const supplier names.
use super::{ModuleAudience, ModuleAudiences, VisibilityDiagnostic, VisibilityRule};
use crate::access_declarations::{Failure, module_scopes::ParsedHeader};
use crate::{DeclarationSurface, SurfaceDeclarationId, SurfaceDeclarationKind};
use nocter_declarations::value_access::{AggregateForm, DeclarationId, PayloadSchema, Program};
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::{AccessFieldId, NominalTypeId, TypeId};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{BoundSyntax, NodeId, NodeKind, direct_node};
use std::collections::{BTreeMap, BTreeSet};

impl ModuleAudiences {
    pub fn verify_signatures(
        &self,
        surface: &DeclarationSurface<'_>,
        declarations: &[Option<DeclarationId>],
        fields: &[Option<AccessFieldId>],
        program: &Program,
        headers: &BTreeMap<SurfaceDeclarationId, ParsedHeader>,
    ) -> Result<(), Failure> {
        let exported_records = surface
            .declarations()
            .iter()
            .enumerate()
            .filter_map(|(index, _)| {
                let raw = SurfaceDeclarationId::from_index(index);
                let audience = self.declaration(raw)?;
                let DeclarationId::Record(record) = declarations[index]? else {
                    return None;
                };
                Some((record, (raw, audience)))
            })
            .collect::<BTreeMap<_, _>>();
        let checking = Signatures {
            surface,
            declarations,
            program,
            exported_records,
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
                        checking.check(audience, syntax, source.ty, parameter.data())?;
                    }
                    checking.check(audience, syntax, header.result_syntax, code.result())?;
                }
                None if declaration.kind() == SurfaceDeclarationKind::Field => {
                    let id = fields[index].ok_or(Failure::InconsistentSyntax)?;
                    let field = program
                        .fields()
                        .get(id)
                        .ok_or(Failure::InconsistentSyntax)?;
                    let ty = direct_node(tree, declaration.node(), NodeKind::Type)
                        .ok_or(Failure::InconsistentSyntax)?;
                    checking.check(audience, syntax, ty, field.data())?;
                }
                Some(DeclarationId::Record(_)) => {}
                None if matches!(
                    declaration.kind(),
                    SurfaceDeclarationKind::TypeBehavior
                        | SurfaceDeclarationKind::StructuralLiteralGrant
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
    exported_records: BTreeMap<NominalTypeId, (SurfaceDeclarationId, &'a ModuleAudience)>,
}
impl Signatures<'_, '_> {
    fn check(
        &self,
        audience: &ModuleAudience,
        syntax: BoundSyntax<'_>,
        primary: NodeId,
        data: TypeId,
    ) -> Result<(), Failure> {
        let mut seen = BTreeSet::new();
        let mut pending = vec![data];
        while let Some(data) = pending.pop() {
            if !seen.insert(data) {
                continue;
            }
            if let Some(record) = self.program.record_identity(data) {
                if self
                    .exported_records
                    .get(&record)
                    .is_none_or(|(_, target)| !audience.is_within(target))
                {
                    return self.reject(syntax, primary, record);
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
    fn reject(
        &self,
        syntax: BoundSyntax<'_>,
        primary: NodeId,
        target: NominalTypeId,
    ) -> Result<(), Failure> {
        let raw = match self.exported_records.get(&target) {
            Some((raw, _)) => *raw,
            None => self
                .declarations
                .iter()
                .position(|id| *id == Some(DeclarationId::Record(target)))
                .map(SurfaceDeclarationId::from_index)
                .ok_or(Failure::InconsistentSyntax)?,
        };
        let declaration = self.surface.declarations()[raw.index()];
        let tree = self.surface.sources()[declaration.source().index()].syntax();
        let token = declaration.name().ok_or(Failure::InconsistentSyntax)?;
        let primary = SourceOrigin::from_node(syntax.tree(), primary)
            .map_err(|_| Failure::InconsistentSyntax)?;
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
