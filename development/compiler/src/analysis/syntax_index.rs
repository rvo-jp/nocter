//! Syntax-only editor sites that have no semantic identity domain yet.
//!
//! Successful definitions and references belong in `SemanticOccurrenceIndex`.
//! This index retains authored query sites whose meaning is their syntax, so
//! editor features do not walk the complete AST independently for each request.

use crate::analysis::editor_targets::{EditorTarget, EditorTargetKind};
use crate::ast::{
    AstFile, CallExpr, ConformanceDecl, ConstructMemberDecl, Expr, FunctionDecl, GenericParamList,
    Item, LiteralDecl, MethodDecl, ModulePath, PrimitiveDecl, TypeExpr, WhereClause,
};
use crate::comments::{AttachedDocumentation, DocumentationTarget, attach_documentation};
use crate::resolve::ResolveOutput;
use crate::semantic::DefId;
use crate::source::ByteSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CoercionRequirementSite {
    pub(crate) focus_span: ByteSpan,
    pub(crate) source: TypeExpr,
    pub(crate) target: TypeExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallCursorRegion {
    FullCall,
    Arguments,
}

#[derive(Debug, Clone)]
pub(crate) struct MethodOwnerSyntax {
    pub(crate) generics: GenericParamList,
    pub(crate) target_ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub(crate) enum CallableSyntax {
    Function(FunctionDecl),
    Primitive(PrimitiveDecl),
    Method {
        owner: MethodOwnerSyntax,
        method: MethodDecl,
    },
    InterfaceMethod(MethodDecl),
    Literal(LiteralDecl),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct EditorSyntaxIndex {
    coercion_requirements: Vec<CoercionRequirementSite>,
    module_paths: Vec<ModulePath>,
    editor_targets: Vec<EditorTarget>,
    calls: Vec<CallExpr>,
    callables: std::collections::HashMap<DefId, CallableSyntax>,
    conformances: Vec<ConformanceDecl>,
    documentation: AttachedDocumentation,
    documentation_owners: Vec<(ByteSpan, ByteSpan)>,
}

impl EditorSyntaxIndex {
    pub(crate) fn new(text: &str, ast: &AstFile, resolved: &ResolveOutput) -> Self {
        let mut coercion_requirements = Vec::new();
        for item in &ast.items {
            for clause in item_requirement_clauses(item).into_iter().flatten() {
                coercion_requirements.extend(clause.coercion_requirements().map(|requirement| {
                    CoercionRequirementSite {
                        focus_span: requirement.as_span,
                        source: requirement.source.clone(),
                        target: requirement.target.clone(),
                    }
                }));
            }
        }
        coercion_requirements.sort_by_key(|site| (site.focus_span.start, site.focus_span.end));
        let mut editor_targets = super::editor_targets::editor_targets_for_ast(ast, resolved);
        editor_targets.sort_by_key(|target| (target.focus_span.start, target.focus_span.end));
        let mut module_paths = editor_targets
            .iter()
            .filter_map(|target| match &target.kind {
                EditorTargetKind::Module(path) => Some(path.clone()),
                EditorTargetKind::ImportBinding(_) => None,
            })
            .collect::<Vec<_>>();
        module_paths.sort_by_key(|path| (path.span.start, path.span.end));
        let mut calls = Vec::new();
        crate::ast::visit_file_expressions(ast, &mut |expression| {
            if let Expr::Call(call) = expression {
                calls.push(call.clone());
            }
        });
        calls.sort_by_key(|call| (call.span.start, call.span.end));
        let mut callables = std::collections::HashMap::new();
        let mut conformances = Vec::new();
        for item in &ast.items {
            collect_callable_syntax(item, resolved, &mut callables);
            if let Item::Conformance(conformance) = item {
                conformances.push(conformance.clone());
            }
        }
        conformances.sort_by_key(|conformance| (conformance.span.start, conformance.span.end));
        let mut documentation_owners = resolved
            .semantic_db
            .definitions()
            .iter()
            .filter(|definition| definition.anchor.source == ast.span.source)
            .filter(|definition| definition_accepts_documentation(definition.kind))
            .map(|definition| (definition.span, definition.anchor))
            .collect::<Vec<_>>();
        documentation_owners.extend(
            resolved
                .local_symbols()
                .filter(|symbol| {
                    symbol.name_span.source == ast.span.source
                        && resolved
                            .semantic_db
                            .definition_at(symbol.name_span)
                            .is_none()
                })
                .map(|symbol| (symbol.name_span, symbol.name_span)),
        );
        documentation_owners
            .sort_by_key(|(span, anchor)| (span.start, span.end, anchor.start, anchor.end));
        documentation_owners.dedup();
        let targets = documentation_owners
            .iter()
            .map(|(span, anchor)| {
                DocumentationTarget::new(declaration_line_start(text, span.start), anchor.start)
            })
            .collect::<Vec<_>>();
        Self {
            coercion_requirements,
            module_paths,
            editor_targets,
            calls,
            callables,
            conformances,
            documentation: attach_documentation(ast.span.source, text, &targets),
            documentation_owners,
        }
    }

    pub(crate) fn module_path_at(&self, offset: usize) -> Option<&ModulePath> {
        self.module_paths
            .iter()
            .filter(|path| contains(path.span, offset))
            .min_by_key(|path| (path.span.len(), path.span.start))
    }

    pub(crate) fn editor_target_at(&self, offset: usize) -> Option<&EditorTarget> {
        self.editor_targets
            .iter()
            .filter(|target| contains(target.focus_span, offset))
            .min_by_key(|target| (target.focus_span.len(), target.focus_span.start))
    }

    pub(crate) fn editor_targets(&self) -> impl Iterator<Item = &EditorTarget> {
        self.editor_targets.iter()
    }

    pub(crate) fn call_at(&self, offset: usize, region: CallCursorRegion) -> Option<&CallExpr> {
        self.calls
            .iter()
            .filter(|call| {
                let span = match region {
                    CallCursorRegion::FullCall => call.span,
                    CallCursorRegion::Arguments => call.arguments_span,
                };
                contains_or_touches(span, offset)
            })
            .min_by_key(|call| (call.span.len(), call.span.start))
    }

    pub(crate) fn callable(&self, definition: DefId) -> Option<&CallableSyntax> {
        self.callables.get(&definition)
    }

    pub(crate) fn conformance_at(&self, offset: usize) -> Option<&ConformanceDecl> {
        self.conformances
            .iter()
            .filter(|conformance| contains_or_touches(conformance.target_ty.span(), offset))
            .min_by_key(|conformance| (conformance.span.len(), conformance.span.start))
    }

    pub(crate) fn coercion_requirement_at(
        &self,
        offset: usize,
    ) -> Option<&CoercionRequirementSite> {
        self.coercion_requirements
            .iter()
            .filter(|site| contains(site.focus_span, offset))
            .min_by_key(|site| (site.focus_span.len(), site.focus_span.start))
    }

    pub(crate) fn documentation_at(&self, target: ByteSpan) -> Option<&str> {
        self.documentation.get(target.start).or_else(|| {
            self.documentation_owners
                .iter()
                .filter(|(span, _)| contains_or_touches(*span, target.start))
                .min_by_key(|(span, _)| (span.len(), span.start))
                .and_then(|(_, anchor)| self.documentation.get(anchor.start))
        })
    }
}

fn collect_callable_syntax(
    item: &Item,
    resolved: &ResolveOutput,
    callables: &mut std::collections::HashMap<DefId, CallableSyntax>,
) {
    let canonical = |anchor| {
        let definition = resolved.semantic_db.definition_at(anchor)?;
        Some(
            resolved
                .callable_bodies
                .declaration_id(definition)
                .unwrap_or(definition),
        )
    };
    match item {
        Item::Function(function) => {
            if let Some(definition) = canonical(function.member_name_span) {
                callables.insert(definition, CallableSyntax::Function(function.clone()));
            }
        }
        Item::Primitive(primitive) => {
            if let Some(definition) = canonical(primitive.name_span) {
                callables.insert(definition, CallableSyntax::Primitive(primitive.clone()));
            }
        }
        Item::Instance(_) | Item::Conformance(_) => {
            let method_owner = item.method_owner().expect("matched method owner");
            let owner = MethodOwnerSyntax {
                generics: method_owner.generics().clone(),
                target_ty: method_owner.target_ty().clone(),
            };
            for method in method_owner.methods() {
                if let Some(definition) = canonical(method.name_span) {
                    callables.insert(
                        definition,
                        CallableSyntax::Method {
                            owner: owner.clone(),
                            method: method.clone(),
                        },
                    );
                }
            }
        }
        Item::Interface(interface) => {
            for method in &interface.methods {
                if let Some(definition) = canonical(method.name_span) {
                    callables.insert(definition, CallableSyntax::InterfaceMethod(method.clone()));
                }
            }
        }
        Item::Construct(construct) => {
            for member in &construct.members {
                match &member.declaration {
                    ConstructMemberDecl::Function(function) => {
                        if let Some(definition) = canonical(function.member_name_span) {
                            callables
                                .insert(definition, CallableSyntax::Function(function.clone()));
                        }
                    }
                    ConstructMemberDecl::Literal(literal) => {
                        if let Some(definition) = canonical(literal.shape_span) {
                            callables.insert(definition, CallableSyntax::Literal(literal.clone()));
                        }
                    }
                }
            }
        }
        Item::Import(_)
        | Item::FromImport(_)
        | Item::Test(_)
        | Item::TypeAlias(_)
        | Item::Struct(_)
        | Item::Enum(_)
        | Item::Destruct(_) => {}
    }
}

fn item_requirement_clauses(item: &Item) -> Vec<Option<&WhereClause>> {
    match item {
        Item::Function(function) => vec![function.requirements.as_ref()],
        Item::Primitive(primitive) => vec![primitive.requirements.as_ref()],
        Item::TypeAlias(alias) => vec![alias.requirements.as_ref()],
        Item::Struct(struct_) => vec![struct_.requirements.as_ref()],
        Item::Enum(enum_) => vec![enum_.requirements.as_ref()],
        Item::Interface(interface) => std::iter::once(interface.requirements.as_ref())
            .chain(
                interface
                    .methods
                    .iter()
                    .map(|method| method.requirements.as_ref()),
            )
            .collect(),
        Item::Instance(_) | Item::Conformance(_) => {
            let owner = item.method_owner().expect("matched method owner");
            std::iter::once(owner.requirements())
                .chain(owner.methods().map(|method| method.requirements.as_ref()))
                .collect()
        }
        Item::Construct(construct) => construct
            .functions()
            .map(|(_, function)| function.requirements.as_ref())
            .chain(
                construct
                    .literals()
                    .map(|(_, literal)| literal.requirements.as_ref()),
            )
            .collect(),
        Item::Import(_) | Item::FromImport(_) | Item::Test(_) | Item::Destruct(_) => Vec::new(),
    }
}

const fn contains(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

const fn contains_or_touches(span: ByteSpan, offset: usize) -> bool {
    span.start <= offset && offset <= span.end
}

fn declaration_line_start(text: &str, node_start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut line_start = node_start.min(bytes.len());
    while line_start > 0 && bytes[line_start - 1] != b'\n' {
        line_start -= 1;
    }
    while line_start < node_start && matches!(bytes[line_start], b' ' | b'\t') {
        line_start += 1;
    }
    line_start
}

const fn definition_accepts_documentation(kind: crate::semantic::DefinitionKind) -> bool {
    !matches!(
        kind,
        crate::semantic::DefinitionKind::GenericParameter
            | crate::semantic::DefinitionKind::Parameter
            | crate::semantic::DefinitionKind::Receiver
            | crate::semantic::DefinitionKind::LiteralCapture
    )
}

#[cfg(test)]
mod tests {
    use crate::analysis::single_file::analyze_single_file_text;

    #[test]
    fn indexes_nested_callable_requirement_sites_once() {
        let text = "interface View { pub method &self.get<T>(): &str where &T as &str }\n";
        let (_sources, analysis) = analyze_single_file_text("syntax.nct", text).unwrap();
        let file = analysis.root_file().unwrap();
        let offset = text.find("as &str").unwrap();
        let site = file.syntax.coercion_requirement_at(offset).unwrap();
        assert_eq!(&text[site.focus_span.start..site.focus_span.end], "as");
    }
}
