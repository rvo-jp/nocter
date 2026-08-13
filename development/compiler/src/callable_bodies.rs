//! Source-backed callable contract and implementation identity.

use crate::ast::{
    AstFile, ConformanceMember, ConstructDecl, ConstructMemberDecl, FunctionDecl, GenericParamList,
    InstanceDecl, Item, LiteralDecl, LiteralShape, MethodDecl, MethodOwnerDecl, ParameterList,
    ResultProvenanceClause, Visibility, canonical_type_expr,
};
use crate::diagnostics::{Diagnostic, DiagnosticNote};
use crate::resolve::ImportSourceMap;
use crate::semantic::{DefId, SemanticDb};
use crate::source::{ByteSpan, SourceId, SourceMap};
use crate::source_modules::SourceModuleMap;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CallableBodyIndex {
    semantic_db: Arc<SemanticDb>,
    declaration_to_implementation: HashMap<DefId, DefId>,
    implementation_to_declaration: HashMap<DefId, DefId>,
    implementation_input_to_declaration: HashMap<DefId, DefId>,
}

impl CallableBodyIndex {
    pub(crate) fn build(
        sources: &SourceMap,
        files: &[AstFile],
        import_sources: &ImportSourceMap,
        semantic_db: Arc<SemanticDb>,
    ) -> (Self, Vec<Diagnostic>) {
        let modules = SourceModuleMap::new(files, import_sources);
        let mut contracts = Vec::new();
        let mut implementations = Vec::new();
        let mut diagnostics = Vec::new();

        for file in files {
            collect_file_callables(
                sources,
                file,
                modules.module(file.span.source).unwrap_or(file.span.source),
                &semantic_db,
                &mut contracts,
                &mut implementations,
                &mut diagnostics,
            );
        }

        contracts.sort_by_key(|callable| span_order(callable.declaration_span));
        implementations.sort_by_key(|callable| span_order(callable.declaration_span));
        let mut index = Self {
            semantic_db,
            ..Self::default()
        };
        for contract in contracts {
            let candidates = implementations
                .iter()
                .filter(|implementation| {
                    implementation.module == contract.module
                        && implementation.key == contract.key
                        && implementation.declaration_span.source
                            != contract.declaration_span.source
                })
                .collect::<Vec<_>>();
            match candidates.as_slice() {
                [] => diagnostics.push(missing_body_diagnostic(sources, &contract)),
                [implementation] if implementation.signature == contract.signature => {
                    let contract_id = contract.definition;
                    let implementation_id = implementation.definition;
                    index
                        .declaration_to_implementation
                        .insert(contract_id, implementation_id);
                    index
                        .implementation_to_declaration
                        .insert(implementation_id, contract_id);
                    for (declaration, implementation) in
                        contract.inputs.iter().zip(&implementation.inputs)
                    {
                        index
                            .implementation_input_to_declaration
                            .insert(*implementation, *declaration);
                    }
                }
                [implementation] => diagnostics.push(signature_mismatch_diagnostic(
                    sources,
                    &contract,
                    implementation,
                )),
                _ => diagnostics.push(duplicate_body_diagnostic(sources, &contract, &candidates)),
            }
        }

        (index, diagnostics)
    }

    pub(crate) fn implementation_id(&self, declaration: DefId) -> Option<DefId> {
        self.declaration_to_implementation
            .get(&declaration)
            .copied()
    }

    pub(crate) fn declaration_id(&self, implementation: DefId) -> Option<DefId> {
        self.implementation_to_declaration
            .get(&implementation)
            .copied()
    }

    pub(crate) fn canonical_definition(&self, definition: DefId) -> DefId {
        self.declaration_id(definition).unwrap_or(definition)
    }

    pub(crate) fn canonical_input_definition(&self, definition: DefId) -> DefId {
        self.implementation_input_to_declaration
            .get(&definition)
            .copied()
            .unwrap_or(definition)
    }

    pub(crate) fn semantic_db(&self) -> Arc<SemanticDb> {
        self.semantic_db.clone()
    }

    pub(crate) fn is_implementation(&self, span: ByteSpan) -> bool {
        self.semantic_db
            .definition_at(span)
            .is_some_and(|definition| self.implementation_to_declaration.contains_key(&definition))
    }

    /// Removes paired private implementation declarations from a module's symbol surface while
    /// retaining their authored AST in the physical file used for body analysis.
    pub(crate) fn declaration_surface(&self, ast: &AstFile) -> AstFile {
        let items = ast
            .items
            .iter()
            .filter_map(|item| self.declaration_surface_item(item))
            .collect();
        AstFile {
            span: ast.span,
            items,
        }
    }

    fn declaration_surface_item(&self, item: &Item) -> Option<Item> {
        match item {
            Item::Function(function) if self.is_implementation(function_identity(function)) => None,
            Item::Instance(instance) => {
                let mut filtered = instance.clone();
                filtered.members.retain(|member| match member {
                    crate::ast::InstanceMember::Method(method) => {
                        !self.is_implementation(method.name_span)
                    }
                    crate::ast::InstanceMember::Operator(operator) => {
                        !self.is_implementation(operator.anchor_span())
                    }
                    crate::ast::InstanceMember::Coercion(entry) => {
                        !self.is_implementation(entry.as_span)
                    }
                });
                Some(Item::Instance(filtered))
            }
            Item::Conformance(conformance) => {
                let mut filtered = conformance.clone();
                filtered.members.retain(|member| match member {
                    ConformanceMember::AssociatedType(_) => true,
                    ConformanceMember::Method(method) => !self.is_implementation(method.name_span),
                });
                Some(Item::Conformance(filtered))
            }
            Item::Construct(construct) => {
                let mut filtered = construct.clone();
                filtered.members.retain(|member| {
                    let identity = match &member.declaration {
                        ConstructMemberDecl::Function(function) => function_identity(function),
                        ConstructMemberDecl::Literal(literal) => literal.shape_span,
                    };
                    !self.is_implementation(identity)
                });
                (!filtered.members.is_empty() || construct.members.is_empty())
                    .then_some(Item::Construct(filtered))
            }
            _ => Some(item.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CallableKey {
    Function {
        owner: Option<String>,
        name: String,
    },
    Method {
        owner: String,
        name: String,
    },
    Operator {
        owner: String,
        shape: OperatorShape,
        receiver: &'static str,
    },
    Literal {
        owner: String,
        shape: LiteralShape,
    },
    Coercion {
        owner: String,
        target: String,
        receiver: &'static str,
    },
}

impl CallableKey {
    fn label(&self) -> String {
        match self {
            Self::Function { owner, name } => owner.as_ref().map_or_else(
                || format!("function `{name}`"),
                |owner| format!("associated function `{owner}.{name}`"),
            ),
            Self::Method { owner, name } => format!("method `{owner}.{name}`"),
            Self::Operator { owner, shape, .. } => {
                format!("{} operator for `{owner}`", shape.label())
            }
            Self::Literal { owner, shape } => format!(
                "{} literal for `{owner}`",
                match shape {
                    LiteralShape::Sequence => "sequence",
                    LiteralShape::String => "string",
                }
            ),
            Self::Coercion { owner, target, .. } => {
                format!("coercion from `{owner}` to `{target}`")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum OperatorShape {
    Equality,
    StrictOrder,
    Index,
    Expansion,
}

impl OperatorShape {
    fn label(self) -> &'static str {
        match self {
            Self::Equality => "equality",
            Self::StrictOrder => "ordering",
            Self::Index => "index",
            Self::Expansion => "expansion",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallableSignature {
    owner_generics: Vec<String>,
    generics: Vec<String>,
    receiver: Option<&'static str>,
    parameters: Vec<String>,
    return_type: String,
    provenance: Vec<String>,
}

#[derive(Debug, Clone)]
struct CallableRecord {
    module: SourceId,
    definition: DefId,
    declaration_span: ByteSpan,
    key: CallableKey,
    signature: CallableSignature,
    inputs: Vec<DefId>,
}

fn collect_file_callables(
    sources: &SourceMap,
    file: &AstFile,
    module: SourceId,
    semantic_db: &SemanticDb,
    contracts: &mut Vec<CallableRecord>,
    implementations: &mut Vec<CallableRecord>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let is_root = sources
        .get(file.span.source)
        .and_then(|source| source.absolute_path())
        .is_none_or(|path| crate::source_layout::is_module_root_source(path));

    for item in &file.items {
        match item {
            Item::Function(function) => classify(
                sources,
                record_for_function(module, function, semantic_db),
                function.visibility,
                function.body.is_some(),
                is_root,
                contracts,
                implementations,
                diagnostics,
            ),
            Item::Instance(instance) => {
                collect_inherent_methods(
                    sources,
                    module,
                    instance,
                    semantic_db,
                    is_root,
                    contracts,
                    implementations,
                    diagnostics,
                );
                collect_coercions(
                    sources,
                    module,
                    instance,
                    semantic_db,
                    is_root,
                    contracts,
                    implementations,
                    diagnostics,
                );
            }
            Item::Conformance(conformance) => {
                for member in &conformance.members {
                    if let ConformanceMember::Method(method) = member
                        && method.body.is_none()
                    {
                        diagnostics.push(invalid_bodyless_diagnostic(
                            sources,
                            method.span,
                            "conformance methods require an inline body",
                        ));
                    }
                }
            }
            Item::Construct(construct) => collect_construct_callables(
                sources,
                module,
                construct,
                semantic_db,
                is_root,
                contracts,
                implementations,
                diagnostics,
            ),
            Item::Import(_)
            | Item::FromImport(_)
            | Item::Test(_)
            | Item::Primitive(_)
            | Item::TypeAlias(_)
            | Item::Struct(_)
            | Item::Enum(_)
            | Item::Interface(_) => {}
            Item::Destruct(_) => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn classify(
    sources: &SourceMap,
    record: CallableRecord,
    visibility: Visibility,
    has_body: bool,
    is_root: bool,
    contracts: &mut Vec<CallableRecord>,
    implementations: &mut Vec<CallableRecord>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match (has_body, visibility, is_root) {
        (false, Visibility::Private, _) => diagnostics.push(invalid_bodyless_diagnostic(
            sources,
            record.declaration_span,
            "a bodyless callable contract must be explicitly public",
        )),
        (false, _, false) => diagnostics.push(invalid_bodyless_diagnostic(
            sources,
            record.declaration_span,
            "a bodyless callable contract is allowed only in `index.nct`",
        )),
        (false, _, true) => contracts.push(record),
        (true, Visibility::Private, _) => implementations.push(record),
        (true, _, _) => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_inherent_methods(
    sources: &SourceMap,
    module: SourceId,
    instance: &InstanceDecl,
    semantic_db: &SemanticDb,
    is_root: bool,
    contracts: &mut Vec<CallableRecord>,
    implementations: &mut Vec<CallableRecord>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for method in instance.named_methods() {
        classify(
            sources,
            record_for_method(module, instance, method, semantic_db),
            method.visibility,
            method.body.is_some(),
            is_root,
            contracts,
            implementations,
            diagnostics,
        );
    }
    for operator in instance.operators() {
        let callable = operator.callable();
        classify(
            sources,
            record_for_operator(module, instance, operator, semantic_db),
            callable.visibility,
            callable.body.is_some(),
            is_root,
            contracts,
            implementations,
            diagnostics,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_construct_callables(
    sources: &SourceMap,
    module: SourceId,
    construct: &ConstructDecl,
    semantic_db: &SemanticDb,
    is_root: bool,
    contracts: &mut Vec<CallableRecord>,
    implementations: &mut Vec<CallableRecord>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for member in &construct.members {
        let (record, visibility, has_body) = match &member.declaration {
            ConstructMemberDecl::Function(function) => (
                record_for_function(module, function, semantic_db),
                function.visibility,
                function.body.is_some(),
            ),
            ConstructMemberDecl::Literal(literal) => (
                record_for_literal(module, literal, semantic_db),
                literal.visibility,
                literal.body.is_some(),
            ),
        };
        if member.default_span.is_some() && visibility == Visibility::Private {
            diagnostics.push(invalid_bodyless_diagnostic(
                sources,
                member.span,
                "a private construction implementation cannot repeat `default`",
            ));
        }
        classify(
            sources,
            record,
            visibility,
            has_body,
            is_root,
            contracts,
            implementations,
            diagnostics,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_coercions(
    sources: &SourceMap,
    module: SourceId,
    instance: &InstanceDecl,
    semantic_db: &SemanticDb,
    is_root: bool,
    contracts: &mut Vec<CallableRecord>,
    implementations: &mut Vec<CallableRecord>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for entry in instance.coercions() {
        let callable = entry.callable();
        classify(
            sources,
            CallableRecord {
                module,
                definition: required_definition_id(semantic_db, entry.as_span, "coercion"),
                declaration_span: entry.span,
                key: CallableKey::Coercion {
                    owner: canonical_type_expr(&instance.target_ty),
                    target: canonical_type_expr(entry.target()),
                    receiver: callable.receiver.mode.label(),
                },
                signature: CallableSignature {
                    owner_generics: generic_signature(&instance.generics),
                    generics: Vec::new(),
                    receiver: Some(callable.receiver.mode.label()),
                    parameters: Vec::new(),
                    return_type: canonical_type_expr(entry.target()),
                    provenance: provenance_signature(callable.result_provenance.as_ref()),
                },
                inputs: vec![required_definition_id(
                    semantic_db,
                    callable.receiver.name_span,
                    "coercion receiver",
                )],
            },
            callable.visibility,
            callable.body.is_some(),
            is_root,
            contracts,
            implementations,
            diagnostics,
        );
    }
}

fn record_for_function(
    module: SourceId,
    function: &FunctionDecl,
    semantic_db: &SemanticDb,
) -> CallableRecord {
    CallableRecord {
        module,
        definition: required_definition_id(semantic_db, function_identity(function), "function"),
        declaration_span: function.span,
        key: CallableKey::Function {
            owner: function.owner.as_ref().map(|owner| owner.name.clone()),
            name: function.member_name.clone(),
        },
        signature: CallableSignature {
            owner_generics: Vec::new(),
            generics: generic_signature(&function.generics),
            receiver: None,
            parameters: parameter_signature(&function.parameters),
            return_type: canonical_type_expr(&function.return_type),
            provenance: provenance_signature(function.result_provenance.as_ref()),
        },
        inputs: function
            .parameters
            .parameters
            .iter()
            .map(|parameter| {
                required_definition_id(semantic_db, parameter.name_span, "function parameter")
            })
            .collect(),
    }
}

fn record_for_method(
    module: SourceId,
    owner_decl: &(impl MethodOwnerDecl + ?Sized),
    method: &MethodDecl,
    semantic_db: &SemanticDb,
) -> CallableRecord {
    let owner = canonical_type_expr(owner_decl.target_ty());
    CallableRecord {
        module,
        definition: required_definition_id(semantic_db, method.name_span, "method"),
        declaration_span: method.span,
        key: CallableKey::Method {
            owner,
            name: method.name.clone(),
        },
        signature: CallableSignature {
            owner_generics: generic_signature(owner_decl.generics()),
            generics: generic_signature(&method.generics),
            receiver: Some(method.receiver.mode.label()),
            parameters: parameter_signature(&method.parameters),
            return_type: canonical_type_expr(&method.return_type),
            provenance: provenance_signature(method.result_provenance.as_ref()),
        },
        inputs: std::iter::once(required_definition_id(
            semantic_db,
            method.receiver.name_span,
            "method receiver",
        ))
        .chain(method.parameters.parameters.iter().map(|parameter| {
            required_definition_id(semantic_db, parameter.name_span, "method parameter")
        }))
        .collect(),
    }
}

fn record_for_operator(
    module: SourceId,
    owner_decl: &InstanceDecl,
    operator: &crate::ast::OperatorDecl,
    semantic_db: &SemanticDb,
) -> CallableRecord {
    let callable = operator.callable();
    let shape = match operator {
        crate::ast::OperatorDecl::Comparison(operator) => match operator.kind {
            crate::ast::ComparisonOperatorKind::Equality => OperatorShape::Equality,
            crate::ast::ComparisonOperatorKind::StrictOrder => OperatorShape::StrictOrder,
        },
        crate::ast::OperatorDecl::Index(_) => OperatorShape::Index,
        crate::ast::OperatorDecl::Expansion(_) => OperatorShape::Expansion,
    };
    CallableRecord {
        module,
        definition: required_definition_id(semantic_db, operator.anchor_span(), "operator"),
        declaration_span: callable.span,
        key: CallableKey::Operator {
            owner: canonical_type_expr(&owner_decl.target_ty),
            shape,
            receiver: callable.receiver.mode.label(),
        },
        signature: CallableSignature {
            owner_generics: generic_signature(&owner_decl.generics),
            generics: generic_signature(&callable.generics),
            receiver: Some(callable.receiver.mode.label()),
            parameters: parameter_signature(&callable.parameters),
            return_type: canonical_type_expr(&callable.return_type),
            provenance: provenance_signature(callable.result_provenance.as_ref()),
        },
        inputs: std::iter::once(required_definition_id(
            semantic_db,
            callable.receiver.name_span,
            "operator receiver",
        ))
        .chain(callable.parameters.parameters.iter().map(|parameter| {
            required_definition_id(semantic_db, parameter.name_span, "operator parameter")
        }))
        .collect(),
    }
}

fn record_for_literal(
    module: SourceId,
    literal: &LiteralDecl,
    semantic_db: &SemanticDb,
) -> CallableRecord {
    let mut parameters = parameter_signature(&literal.parameters);
    if let Some(capture) = &literal.capture {
        parameters.push(format!(
            "...{}:{}",
            capture.name,
            canonical_type_expr(&capture.element_type)
        ));
    }
    let inputs = literal
        .parameters
        .parameters
        .iter()
        .map(|parameter| {
            required_definition_id(semantic_db, parameter.name_span, "literal parameter")
        })
        .chain(literal.capture.iter().map(|capture| {
            required_definition_id(semantic_db, capture.name_span, "literal capture")
        }))
        .collect();
    CallableRecord {
        module,
        definition: required_definition_id(semantic_db, literal.shape_span, "literal"),
        declaration_span: literal.span,
        key: CallableKey::Literal {
            owner: canonical_type_expr(&literal.target),
            shape: literal.shape,
        },
        signature: CallableSignature {
            owner_generics: Vec::new(),
            generics: Vec::new(),
            receiver: None,
            parameters,
            return_type: canonical_type_expr(&literal.return_type),
            provenance: provenance_signature(literal.result_provenance.as_ref()),
        },
        inputs,
    }
}

fn required_definition_id(
    semantic_db: &SemanticDb,
    location: ByteSpan,
    description: &str,
) -> DefId {
    semantic_db
        .definition_at(location)
        .unwrap_or_else(|| panic!("semantic database omitted {description} at {location:?}"))
}

fn function_identity(function: &FunctionDecl) -> ByteSpan {
    if function.owner.is_some() {
        function.member_name_span
    } else {
        function.name_span
    }
}

fn generic_signature(generics: &GenericParamList) -> Vec<String> {
    generics
        .parameters
        .iter()
        .map(|parameter| parameter.name.clone())
        .collect()
}

fn parameter_signature(parameters: &ParameterList) -> Vec<String> {
    parameters
        .parameters
        .iter()
        .map(|parameter| format!("{}:{}", parameter.name, canonical_type_expr(&parameter.ty)))
        .collect()
}

fn provenance_signature(clause: Option<&ResultProvenanceClause>) -> Vec<String> {
    clause.map_or_else(Vec::new, |clause| {
        clause
            .origins
            .iter()
            .map(|origin| origin.kind.source_label().to_string())
            .collect()
    })
}

fn missing_body_diagnostic(sources: &SourceMap, contract: &CallableRecord) -> Diagnostic {
    Diagnostic::error(
        "E0250",
        format!(
            "{} contract has no implementation body",
            contract.key.label()
        ),
    )
    .with_primary_span_if_absent(sources, contract.declaration_span)
}

fn signature_mismatch_diagnostic(
    sources: &SourceMap,
    contract: &CallableRecord,
    implementation: &CallableRecord,
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0251",
        format!(
            "{} implementation does not exactly match its public contract",
            contract.key.label()
        ),
    )
    .with_primary_span_if_absent(sources, implementation.declaration_span);
    diagnostic.notes.push(note(
        sources,
        contract.declaration_span,
        "public contract is declared here",
    ));
    diagnostic
}

fn duplicate_body_diagnostic(
    sources: &SourceMap,
    contract: &CallableRecord,
    implementations: &[&CallableRecord],
) -> Diagnostic {
    let mut diagnostic = Diagnostic::error(
        "E0252",
        format!(
            "{} contract has multiple implementation bodies",
            contract.key.label()
        ),
    )
    .with_primary_span_if_absent(sources, contract.declaration_span);
    for implementation in implementations {
        diagnostic.notes.push(note(
            sources,
            implementation.declaration_span,
            "candidate implementation is declared here",
        ));
    }
    diagnostic
}

fn invalid_bodyless_diagnostic(
    sources: &SourceMap,
    span: ByteSpan,
    message: &'static str,
) -> Diagnostic {
    Diagnostic::error("E0253", message).with_primary_span_if_absent(sources, span)
}

fn note(sources: &SourceMap, span: ByteSpan, message: &'static str) -> DiagnosticNote {
    DiagnosticNote {
        message: message.to_string(),
        span: sources.span_to_json(span).ok(),
    }
}

fn span_order(span: ByteSpan) -> (u32, usize, usize) {
    (span.source.raw(), span.start, span.end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse;

    #[test]
    fn joins_matching_function_contract_and_body() {
        let mut sources = SourceMap::new();
        let root = sources.add_source("index.nct", None, "pub func answer(value: i32): i32\n");
        let implementation = sources.add_source(
            "answer.nct",
            None,
            "func answer(value: i32): i32 { return value }\n",
        );
        let root_tokens = lex(&sources, root);
        let implementation_tokens = lex(&sources, implementation);
        let root_ast = parse(&sources, root, &root_tokens.tokens).ast.unwrap();
        let implementation_ast = parse(&sources, implementation, &implementation_tokens.tokens)
            .ast
            .unwrap();
        let import_span = ByteSpan::new(root, 0, 1);
        let imports = HashMap::from([(
            import_span,
            crate::resolve::ImportSource {
                source: implementation,
                access: crate::resolve::ImportAccess::Public,
                kind: crate::resolve::ImportKind::Source,
            },
        )]);
        let files = vec![root_ast, implementation_ast];
        let semantic_db = Arc::new(SemanticDb::from_files(&files));
        let (index, diagnostics) =
            CallableBodyIndex::build(&sources, &files, &imports, semantic_db);
        assert!(diagnostics.is_empty());
        let declaration = match &files[0].items[0] {
            Item::Function(function) => function.name_span,
            _ => panic!("expected function"),
        };
        let body = match &files[1].items[0] {
            Item::Function(function) => function.name_span,
            _ => panic!("expected function"),
        };
        let declaration_id = index.semantic_db.definition_at(declaration).unwrap();
        let implementation_id = index.semantic_db.definition_at(body).unwrap();
        assert_eq!(
            index.implementation_id(declaration_id),
            Some(implementation_id)
        );
        assert_eq!(
            index.declaration_id(implementation_id),
            Some(declaration_id)
        );
        assert_eq!(
            index.canonical_definition(implementation_id),
            declaration_id
        );
        assert_eq!(
            index.declaration_to_implementation.get(&declaration_id),
            Some(&implementation_id)
        );
        assert_eq!(
            index.implementation_to_declaration.get(&implementation_id),
            Some(&declaration_id)
        );
        let declaration_input = match &files[0].items[0] {
            Item::Function(function) => function.parameters.parameters[0].name_span,
            _ => panic!("expected function"),
        };
        let body_input = match &files[1].items[0] {
            Item::Function(function) => function.parameters.parameters[0].name_span,
            _ => panic!("expected function"),
        };
        let declaration_input_id = index.semantic_db.definition_at(declaration_input).unwrap();
        let body_input_id = index.semantic_db.definition_at(body_input).unwrap();
        assert_eq!(
            index.canonical_input_definition(body_input_id),
            declaration_input_id
        );
        assert_eq!(
            index
                .implementation_input_to_declaration
                .get(&body_input_id),
            Some(&declaration_input_id)
        );
    }
}
