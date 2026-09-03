use std::collections::{BTreeMap, HashMap};

use nocter_compile_input::CompileUnitInput;
use nocter_declarations::{BodyOwner, DeclarationGraph, ExportedEntity};
use nocter_frontend_bindings::FrontendBindings;
use nocter_model::{
    ArenaBuilder, BodyScopeId, CaptureId, LocalBindingId, ModuleId, ParameterId, Symbol,
};
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole};
use nocter_syntax::SyntaxOrigin;
use nocter_syntax::{
    Keyword, NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, TokenKind,
};

use super::diagnostic;
use super::model::{
    BodyScope, Capture, CaptureMode, LocalBinding, LocalBindingKind, NameTarget,
    ResolvedBindingOrigins, ResolvedBodyNames, ResolvedNameUse, ScopeBinding,
};
use super::{NameResolutionError, NameResolutionInternalError, Projection};
use crate::BodySource;
use crate::syntax::{
    child_nodes, descendant_identifiers, direct_identifier, direct_node, direct_nodes,
    outermost_descendants, token_symbol, token_text,
};

pub(super) struct ResolvedBody {
    pub(super) body: ResolvedBodyNames,
    pub(super) projections: Vec<Projection>,
}

pub(super) struct BodyResolutionFailure {
    pub(super) error: Box<NameResolutionError>,
    pub(super) partial: Option<Box<ResolvedBody>>,
}

#[derive(Clone, Copy)]
struct ActiveBinding {
    target: NameTarget,
    origin: Option<SourceOrigin>,
}

struct ActiveScope {
    id: BodyScopeId,
    names: BTreeMap<Symbol, ActiveBinding>,
}

#[derive(Clone, Copy)]
struct Introduction {
    token: SyntaxToken,
    kind: LocalBindingKind,
    documentation: Option<NodeId>,
}

enum Action {
    Visit(NodeId),
    EnterBlock {
        block: NodeId,
        introductions: Vec<Introduction>,
    },
    ExitScope,
    Declare(Introduction),
    EnterClosure(NodeId),
    ExitClosure {
        outer_scope_count: usize,
    },
}

pub(super) struct BodyNameResolver<'input, 'syntax> {
    input: &'input CompileUnitInput<'syntax>,
    graph: &'input DeclarationGraph,
    bindings: &'input FrontendBindings,
    source: BodySource<'syntax>,
    scopes: ArenaBuilder<BodyScopeId, BodyScope>,
    locals: ArenaBuilder<LocalBindingId, LocalBinding>,
    captures: ArenaBuilder<CaptureId, Capture>,
    local_origins: ArenaBuilder<LocalBindingId, SyntaxOrigin>,
    capture_origins: ArenaBuilder<CaptureId, SyntaxOrigin>,
    block_scopes: HashMap<NodeId, BodyScopeId>,
    active: Vec<ActiveScope>,
    callable_boundaries: Vec<usize>,
    uses: Vec<ResolvedNameUse>,
    projections: Vec<Projection>,
}

impl<'input, 'syntax> BodyNameResolver<'input, 'syntax> {
    pub(super) fn new(
        input: &'input CompileUnitInput<'syntax>,
        graph: &'input DeclarationGraph,
        bindings: &'input FrontendBindings,
        source: BodySource<'syntax>,
    ) -> Self {
        Self {
            input,
            graph,
            bindings,
            source,
            scopes: ArenaBuilder::new(),
            locals: ArenaBuilder::new(),
            captures: ArenaBuilder::new(),
            local_origins: ArenaBuilder::new(),
            capture_origins: ArenaBuilder::new(),
            block_scopes: HashMap::new(),
            active: Vec::new(),
            callable_boundaries: Vec::new(),
            uses: Vec::new(),
            projections: Vec::new(),
        }
    }

    pub(super) fn resolve_recovering(mut self) -> Result<ResolvedBody, BodyResolutionFailure> {
        match self.resolve_active() {
            Ok(()) => Ok(self.finish()),
            Err(error) => {
                let partial =
                    matches!(error, NameResolutionError::Rule(_)).then(|| Box::new(self.finish()));
                Err(BodyResolutionFailure {
                    error: Box::new(error),
                    partial,
                })
            }
        }
    }

    fn resolve_active(&mut self) -> Result<(), NameResolutionError> {
        let root_scope = self.push_scope(None);
        self.record_block_scope(self.source.block(), root_scope)?;
        self.callable_boundaries.push(0);
        self.seed_parameters()?;
        let mut actions = Vec::new();
        actions.push(Action::ExitClosure {
            outer_scope_count: 0,
        });
        self.schedule_block_contents(self.source.block(), &mut actions)?;
        self.run(actions)?;
        if !self.active.is_empty() || !self.callable_boundaries.is_empty() {
            return Err(NameResolutionInternalError::InvalidSyntaxNode(self.source.block()).into());
        }
        Ok(())
    }

    fn finish(self) -> ResolvedBody {
        ResolvedBody {
            body: ResolvedBodyNames::new(
                self.source.body(),
                self.scopes.finish(),
                self.locals.finish(),
                self.captures.finish(),
                ResolvedBindingOrigins {
                    locals: self.local_origins.finish(),
                    captures: self.capture_origins.finish(),
                },
                self.block_scopes,
                self.uses,
            ),
            projections: self.projections,
        }
    }

    fn run(&mut self, mut actions: Vec<Action>) -> Result<(), NameResolutionError> {
        while let Some(action) = actions.pop() {
            match action {
                Action::Visit(node) => self.visit(node, &mut actions)?,
                Action::EnterBlock {
                    block,
                    introductions,
                } => {
                    let parent = self.active.last().map(|scope| scope.id);
                    let scope = self.push_scope(parent);
                    self.record_block_scope(block, scope)?;
                    for introduction in introductions {
                        self.declare_local(introduction)?;
                    }
                    actions.push(Action::ExitScope);
                    self.schedule_block_contents(block, &mut actions)?;
                }
                Action::ExitScope => {
                    self.active.pop().ok_or_else(|| {
                        NameResolutionInternalError::InvalidSyntaxNode(self.source.block())
                    })?;
                }
                Action::Declare(introduction) => self.declare_local(introduction)?,
                Action::EnterClosure(node) => self.enter_closure(node, &mut actions)?,
                Action::ExitClosure { outer_scope_count } => {
                    if self.active.len() != outer_scope_count + 1 {
                        return Err(NameResolutionInternalError::InvalidSyntaxNode(
                            node_for_error(self.source),
                        )
                        .into());
                    }
                    self.active.pop();
                    self.callable_boundaries.pop();
                }
            }
        }
        Ok(())
    }

    fn visit(
        &mut self,
        node: NodeId,
        actions: &mut Vec<Action>,
    ) -> Result<(), NameResolutionError> {
        let kind = self.node_kind(node)?;
        match kind {
            NodeKind::Block => actions.push(Action::EnterBlock {
                block: node,
                introductions: Vec::new(),
            }),
            NodeKind::BindingStatement => self.visit_binding(node, actions)?,
            NodeKind::ForStatement => self.visit_for(node, actions)?,
            NodeKind::RegionStatement => self.visit_region(node, actions)?,
            NodeKind::IfExpression => self.visit_if(node, actions)?,
            NodeKind::MatchExpression => self.visit_match(node, actions),
            NodeKind::MatchArm => self.visit_match_arm(node, actions)?,
            NodeKind::RecoveryClause => self.visit_recovery_clause(node, actions)?,
            NodeKind::ClosureExpression => actions.push(Action::EnterClosure(node)),
            NodeKind::ReferenceExpression | NodeKind::NamedPlace | NodeKind::DropStatement => {
                self.resolve_node_name(node)?;
            }
            NodeKind::NamedType => {
                self.resolve_type_override(node)?;
                for child in child_nodes(self.tree(), node).into_iter().rev() {
                    actions.push(Action::Visit(child));
                }
            }
            NodeKind::PostfixExpression if self.resolve_module_member(node)? => {}
            NodeKind::BlockUseDeclaration => {}
            _ => {
                for child in child_nodes(self.tree(), node).into_iter().rev() {
                    actions.push(Action::Visit(child));
                }
            }
        }
        Ok(())
    }

    /// Resolves a module-qualified chain as exact semantic identities before typed planning.
    ///
    /// Value fields, methods, and construction members remain type-directed, so resolving their
    /// simple owner is the only name work performed here. A valid module member is source-name
    /// resolution, not member dispatch: recording it here gives checking and editor projection one
    /// shared target instead of making the call checker repeat namespace lookup.
    fn resolve_module_member(&mut self, node: NodeId) -> Result<bool, NameResolutionError> {
        let mut current = node;
        let mut members = Vec::new();
        loop {
            let children = child_nodes(self.tree(), current);
            let [owner, member] = children.as_slice() else {
                return Ok(false);
            };
            if self.node_kind(*member)? != NodeKind::MemberSuffix {
                return Ok(false);
            }
            members.push(*member);
            current = *owner;
            if self.node_kind(current)? != NodeKind::PostfixExpression {
                break;
            }
        }
        if self.node_kind(current)? != NodeKind::ReferenceExpression {
            return Ok(false);
        }

        self.resolve_node_name(current)?;
        let owner_token = direct_identifier(self.tree(), current)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(current))?;
        let owner_origin = SyntaxOrigin::Token(owner_token);
        let Some(mut target) = self
            .uses
            .iter()
            .rev()
            .find(|usage| usage.origin() == owner_origin)
            .map(|usage| usage.target())
        else {
            return Ok(true);
        };
        members.reverse();
        for member in members {
            let NameTarget::Exported(ExportedEntity::Module(module)) = target else {
                break;
            };
            let member_token = direct_identifier(self.tree(), member)
                .ok_or(NameResolutionInternalError::InvalidSyntaxNode(member))?;
            let selected = self.resolve_exported_module_member(module, member_token)?;
            target = NameTarget::Exported(selected);
            self.record_use(member_token, target)?;
        }
        Ok(true)
    }

    fn resolve_exported_module_member(
        &self,
        module: ModuleId,
        token: SyntaxToken,
    ) -> Result<ExportedEntity, NameResolutionError> {
        let name = self.symbol(token)?;
        let Some(entry) = self
            .graph
            .module_namespaces()
            .get(module)
            .and_then(|namespace| namespace.lookup_authored(name))
        else {
            return Err(diagnostic::missing_module_member(
                self.spelling(name)?,
                self.origin(token)?,
            )
            .into());
        };
        if !self
            .graph
            .is_visible_from(entry.visibility(), self.source.module(), module)
        {
            return Err(diagnostic::inaccessible_module_member(
                self.spelling(name)?,
                self.origin(token)?,
            )
            .into());
        }
        Ok(entry.target())
    }

    fn visit_binding(
        &self,
        node: NodeId,
        actions: &mut Vec<Action>,
    ) -> Result<(), NameResolutionInternalError> {
        let pattern = direct_node(self.tree(), node, NodeKind::BindingPattern)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        let kind = if self.tree().children(node).iter().any(|element| {
            matches!(
                element,
                SyntaxElement::Token(token)
                    if token.kind() == TokenKind::Keyword(Keyword::Var)
            )
        }) {
            LocalBindingKind::Mutable
        } else {
            LocalBindingKind::Immutable
        };
        let mut introductions = Vec::new();
        self.collect_binding_introductions(pattern, node, kind, &mut introductions)?;
        // Actions are evaluated as a stack. Reversing declarations preserves the pattern's source
        // order after the initializer and annotation have been resolved.
        actions.extend(introductions.into_iter().rev().map(Action::Declare));
        if let Some(annotation) = direct_node(self.tree(), node, NodeKind::TypeAnnotation) {
            actions.push(Action::Visit(annotation));
        }
        let expression = direct_node(self.tree(), node, NodeKind::Expression)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        actions.push(Action::Visit(expression));
        Ok(())
    }

    fn collect_binding_introductions(
        &self,
        pattern: NodeId,
        documentation: NodeId,
        kind: LocalBindingKind,
        introductions: &mut Vec<Introduction>,
    ) -> Result<(), NameResolutionInternalError> {
        let children = direct_nodes(self.tree(), pattern, NodeKind::BindingPattern);
        if children.is_empty() {
            let token = direct_identifier(self.tree(), pattern)
                .ok_or(NameResolutionInternalError::InvalidSyntaxNode(pattern))?;
            introductions.push(Introduction {
                token,
                kind,
                documentation: Some(documentation),
            });
            return Ok(());
        }
        for child in children {
            self.collect_binding_introductions(child, documentation, kind, introductions)?;
        }
        Ok(())
    }

    fn visit_for(
        &self,
        node: NodeId,
        actions: &mut Vec<Action>,
    ) -> Result<(), NameResolutionInternalError> {
        let bindings = direct_node(self.tree(), node, NodeKind::ForBindings)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        let tokens = descendant_identifiers(self.tree(), bindings);
        if tokens.is_empty() || tokens.len() > 2 {
            return Err(NameResolutionInternalError::InvalidSyntaxNode(bindings));
        }
        let source = direct_node(self.tree(), node, NodeKind::ForSource)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        let block = direct_node(self.tree(), node, NodeKind::Block)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        actions.push(Action::EnterBlock {
            block,
            introductions: tokens
                .into_iter()
                .map(|token| Introduction {
                    token,
                    kind: LocalBindingKind::Loop,
                    documentation: None,
                })
                .collect(),
        });
        actions.push(Action::Visit(source));
        Ok(())
    }

    fn visit_region(
        &self,
        node: NodeId,
        actions: &mut Vec<Action>,
    ) -> Result<(), NameResolutionInternalError> {
        let token = direct_identifier(self.tree(), node)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        let allocator = direct_node(self.tree(), node, NodeKind::AllocatorPlace)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        let block = direct_node(self.tree(), node, NodeKind::Block)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        actions.push(Action::EnterBlock {
            block,
            introductions: vec![Introduction {
                token,
                kind: LocalBindingKind::Region,
                documentation: None,
            }],
        });
        actions.push(Action::Visit(allocator));
        Ok(())
    }

    fn visit_if(
        &self,
        node: NodeId,
        actions: &mut Vec<Action>,
    ) -> Result<(), NameResolutionInternalError> {
        let nodes = child_nodes(self.tree(), node);
        let condition = nodes
            .first()
            .copied()
            .filter(|node| self.node_kind(*node).ok() == Some(NodeKind::IfCondition))
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        for child in nodes.iter().skip(1).rev().copied() {
            let introductions = if self.node_kind(child)? == NodeKind::Block {
                self.pattern_introductions(condition)?
            } else {
                Vec::new()
            };
            if self.node_kind(child)? == NodeKind::Block {
                actions.push(Action::EnterBlock {
                    block: child,
                    introductions,
                });
            } else {
                actions.push(Action::Visit(child));
            }
        }
        actions.push(Action::Visit(condition));
        Ok(())
    }

    fn visit_match(&self, node: NodeId, actions: &mut Vec<Action>) {
        let nodes = child_nodes(self.tree(), node);
        for child in nodes.into_iter().rev() {
            actions.push(Action::Visit(child));
        }
    }

    fn visit_match_arm(
        &self,
        node: NodeId,
        actions: &mut Vec<Action>,
    ) -> Result<(), NameResolutionInternalError> {
        let block = direct_node(self.tree(), node, NodeKind::Block)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        actions.push(Action::EnterBlock {
            block,
            introductions: self.pattern_introductions(node)?,
        });
        Ok(())
    }

    fn visit_recovery_clause(
        &self,
        node: NodeId,
        actions: &mut Vec<Action>,
    ) -> Result<(), NameResolutionInternalError> {
        let block = direct_node(self.tree(), node, NodeKind::Block)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        let introductions = direct_identifier(self.tree(), node)
            .filter(|token| token_text(self.input.sources(), *token).ok() != Some("_"))
            .map(|token| {
                vec![Introduction {
                    token,
                    kind: LocalBindingKind::Catch,
                    documentation: None,
                }]
            })
            .unwrap_or_default();
        actions.push(Action::EnterBlock {
            block,
            introductions,
        });
        Ok(())
    }

    fn enter_closure(
        &mut self,
        node: NodeId,
        actions: &mut Vec<Action>,
    ) -> Result<(), NameResolutionError> {
        let head = direct_node(self.tree(), node, NodeKind::ClosureHead)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        let block = direct_node(self.tree(), node, NodeKind::Block)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        let capture_nodes = direct_node(self.tree(), head, NodeKind::ClosureCaptures)
            .map(|captures| direct_nodes(self.tree(), captures, NodeKind::ClosureCapture))
            .unwrap_or_default();
        let parameter_nodes = direct_node(self.tree(), head, NodeKind::ClosureParameters)
            .map(|parameters| direct_nodes(self.tree(), parameters, NodeKind::ClosureParameter))
            .unwrap_or_default();

        let outer_scope_count = self.active.len();
        let mut captures = Vec::with_capacity(capture_nodes.len());
        let mut captured_names = BTreeMap::new();
        for capture_node in capture_nodes {
            let token = direct_identifier(self.tree(), capture_node)
                .ok_or(NameResolutionInternalError::InvalidSyntaxNode(capture_node))?;
            let name = self.symbol(token)?;
            let primary = self.origin(token)?;
            if let Some(first) = captured_names.insert(name, primary) {
                return Err(diagnostic::capture_collision(
                    self.spelling(name)?,
                    primary,
                    Some(first),
                )
                .into());
            }
            let Some(binding) = self.lookup_current_callable(name) else {
                return Err(
                    diagnostic::invalid_capture_target(self.spelling(name)?, primary).into(),
                );
            };
            if !binding.target.is_callable_binding() {
                return Err(
                    diagnostic::invalid_capture_target(self.spelling(name)?, primary).into(),
                );
            }
            captures.push((
                name,
                token,
                binding.target,
                self.capture_mode(capture_node)?,
            ));
        }

        self.callable_boundaries.push(outer_scope_count);
        let scope = self.push_scope(None);
        self.record_block_scope(block, scope)?;
        for (name, token, source, mode) in captures {
            let origin = self.origin(token)?;
            let id = self
                .captures
                .insert(Capture::new(name, scope, source, mode));
            let origin_id = self.capture_origins.insert(origin.syntax());
            if origin_id != id {
                return Err(NameResolutionInternalError::InvalidSyntaxNode(block).into());
            }
            self.current_names_mut()?.insert(
                name,
                ActiveBinding {
                    target: NameTarget::Capture(id),
                    origin: Some(origin),
                },
            );
            self.record_scope_binding(scope, name, NameTarget::Capture(id))?;
            self.projections.push(Projection::new(
                SemanticEntity::Capture(self.source.body(), id),
                SourceRole::Declaration,
                origin,
            ));
        }
        for parameter in parameter_nodes {
            let token = direct_identifier(self.tree(), parameter)
                .ok_or(NameResolutionInternalError::InvalidSyntaxNode(parameter))?;
            let name = self.symbol(token)?;
            if let Some(capture) = captured_names.get(&name).copied() {
                return Err(diagnostic::capture_collision(
                    self.spelling(name)?,
                    self.origin(token)?,
                    Some(capture),
                )
                .into());
            }
            self.declare_local(Introduction {
                token,
                kind: LocalBindingKind::ClosureParameter,
                documentation: None,
            })?;
        }

        actions.push(Action::ExitClosure { outer_scope_count });
        self.schedule_block_contents(block, actions)?;
        actions.push(Action::Visit(head));
        Ok(())
    }

    fn schedule_block_contents(
        &mut self,
        block: NodeId,
        actions: &mut Vec<Action>,
    ) -> Result<(), NameResolutionError> {
        let sequence = direct_node(self.tree(), block, NodeKind::ExecutableSequence);
        if let Some(sequence) = sequence {
            actions.push(Action::Visit(sequence));
        }
        for import in direct_nodes(self.tree(), block, NodeKind::BlockUseDeclaration) {
            self.declare_block_import(import)?;
        }
        Ok(())
    }

    fn declare_block_import(&mut self, node: NodeId) -> Result<(), NameResolutionError> {
        let target_module = self
            .bindings
            .block_import(node)
            .ok_or(NameResolutionInternalError::MissingUseResolution(node))?;
        let path = direct_node(self.tree(), node, NodeKind::ModulePath)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        self.projections.push(Projection::new(
            SemanticEntity::Module(target_module),
            SourceRole::Reference,
            SourceOrigin::from_node(self.tree(), path).map_err(|_| {
                NameResolutionInternalError::InvalidSyntaxOrigin(SyntaxOrigin::Node(path))
            })?,
        ));
        if let Some(selection) = direct_node(self.tree(), node, NodeKind::ImportSelection) {
            for selected in direct_nodes(self.tree(), selection, NodeKind::SelectedName) {
                let tokens = descendant_identifiers(self.tree(), selected);
                let exported = *tokens
                    .first()
                    .ok_or(NameResolutionInternalError::InvalidSyntaxNode(selected))?;
                let local = *tokens
                    .last()
                    .ok_or(NameResolutionInternalError::InvalidSyntaxNode(selected))?;
                let exported_name = self.symbol(exported)?;
                let local_name = self.symbol(local)?;
                let Some(entry) = self
                    .graph
                    .module_namespaces()
                    .get(target_module)
                    .and_then(|namespace| namespace.lookup_authored(exported_name))
                else {
                    return Err(diagnostic::missing_block_import(
                        self.spelling(exported_name)?,
                        self.origin(exported)?,
                    )
                    .into());
                };
                if !self.graph.is_visible_from(
                    entry.visibility(),
                    self.source.module(),
                    target_module,
                ) {
                    return Err(diagnostic::inaccessible_block_import(
                        self.spelling(exported_name)?,
                        self.origin(exported)?,
                    )
                    .into());
                }
                if !entry.target().is_selectable_type() {
                    return Err(diagnostic::non_type_selection(
                        self.spelling(exported_name)?,
                        self.origin(exported)?,
                    )
                    .into());
                }
                let target = NameTarget::Exported(entry.target());
                self.insert_authored_name(local_name, local, target, false)?;
                self.project_use(exported, target)?;
                if local != exported {
                    self.project_use(local, target)?;
                }
            }
        } else {
            let token = if let Some(alias) = direct_node(self.tree(), node, NodeKind::ModuleAlias) {
                *descendant_identifiers(self.tree(), alias)
                    .first()
                    .ok_or(NameResolutionInternalError::InvalidSyntaxNode(alias))?
            } else {
                *descendant_identifiers(self.tree(), path)
                    .last()
                    .ok_or(NameResolutionInternalError::InvalidSyntaxNode(path))?
            };
            let name = self.symbol(token)?;
            let target = NameTarget::Exported(ExportedEntity::Module(target_module));
            self.insert_authored_name(name, token, target, false)?;
            self.project_use(token, target)?;
        }
        Ok(())
    }

    fn seed_parameters(&mut self) -> Result<(), NameResolutionError> {
        let declarations = self.graph.declarations();
        let parameters: Vec<ParameterId> = match self.source.owner() {
            BodyOwner::Callable(owner) => {
                let callable = declarations.callables().get(owner).ok_or(
                    NameResolutionInternalError::InvalidBodyOwner(self.source.body()),
                )?;
                callable
                    .receiver()
                    .into_iter()
                    .chain(callable.parameters().iter().copied())
                    .collect()
            }
            BodyOwner::Drop(owner) => vec![
                declarations
                    .drops()
                    .get(owner)
                    .ok_or(NameResolutionInternalError::InvalidBodyOwner(
                        self.source.body(),
                    ))?
                    .receiver(),
            ],
            BodyOwner::Test(_) => Vec::new(),
        };
        for parameter in parameters {
            let declaration = declarations.parameters().get(parameter).copied().ok_or(
                NameResolutionInternalError::InvalidBodyOwner(self.source.body()),
            )?;
            let origin = self.parameter_origin(parameter)?;
            self.insert_name(
                declaration.name(),
                origin,
                NameTarget::Parameter(parameter),
                false,
            )?;
        }
        Ok(())
    }

    fn declare_local(&mut self, introduction: Introduction) -> Result<(), NameResolutionError> {
        if token_text(self.input.sources(), introduction.token)? == "_" {
            return Ok(());
        }
        let name = self.symbol(introduction.token)?;
        let origin = self.origin(introduction.token)?;
        self.check_collision(name, origin, false)?;
        let scope = self.active.last().map(|scope| scope.id).ok_or(
            NameResolutionInternalError::InvalidSyntaxNode(self.source.block()),
        )?;
        let id = self
            .locals
            .insert(LocalBinding::new(name, scope, introduction.kind));
        let origin_id = self.local_origins.insert(origin.syntax());
        if origin_id != id {
            return Err(NameResolutionInternalError::InvalidSyntaxNode(self.source.block()).into());
        }
        self.current_names_mut()?.insert(
            name,
            ActiveBinding {
                target: NameTarget::Local(id),
                origin: Some(origin),
            },
        );
        self.record_scope_binding(scope, name, NameTarget::Local(id))?;
        let mut projection = Projection::new(
            SemanticEntity::LocalBinding(self.source.body(), id),
            SourceRole::Declaration,
            origin,
        );
        if let Some(node) = introduction.documentation
            && let Some(markdown) = self.tree().documentation(node)
        {
            projection = projection.with_documentation(markdown);
        }
        self.projections.push(projection);
        Ok(())
    }

    fn insert_authored_name(
        &mut self,
        name: Symbol,
        token: SyntaxToken,
        target: NameTarget,
        capture_collision: bool,
    ) -> Result<(), NameResolutionError> {
        let origin = self.origin(token)?;
        self.insert_name(name, origin, target, capture_collision)
    }

    fn insert_name(
        &mut self,
        name: Symbol,
        origin: SourceOrigin,
        target: NameTarget,
        capture_collision: bool,
    ) -> Result<(), NameResolutionError> {
        self.check_collision(name, origin, capture_collision)?;
        let scope = self.active.last().map(|scope| scope.id).ok_or(
            NameResolutionInternalError::InvalidSyntaxNode(self.source.block()),
        )?;
        self.current_names_mut()?.insert(
            name,
            ActiveBinding {
                target,
                origin: Some(origin),
            },
        );
        self.record_scope_binding(scope, name, target)?;
        Ok(())
    }

    fn record_scope_binding(
        &mut self,
        scope: BodyScopeId,
        name: Symbol,
        target: NameTarget,
    ) -> Result<(), NameResolutionInternalError> {
        self.scopes
            .get_mut(scope)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(
                self.source.block(),
            ))?
            .add_binding(ScopeBinding::new(name, target));
        Ok(())
    }

    fn check_collision(
        &self,
        name: Symbol,
        primary: SourceOrigin,
        capture_collision: bool,
    ) -> Result<(), NameResolutionError> {
        if let Some(existing) = self.lookup_all_scopes(name) {
            let diagnostic =
                if capture_collision || matches!(existing.target, NameTarget::Capture(_)) {
                    diagnostic::capture_collision(self.spelling(name)?, primary, existing.origin)
                } else {
                    diagnostic::name_collision(self.spelling(name)?, primary, existing.origin)
                };
            return Err(diagnostic.into());
        }
        let source_namespaces = self.bindings.source_namespaces();
        if source_namespaces
            .lookup_authored(self.tree().source(), name)
            .is_some()
            || matches!(
                source_namespaces.lookup(self.tree().source(), name),
                Some(ExportedEntity::BuiltinType(_))
            )
            || self.spelling(name)? == "Self"
        {
            return Err(diagnostic::name_collision(self.spelling(name)?, primary, None).into());
        }
        Ok(())
    }

    fn resolve_node_name(&mut self, node: NodeId) -> Result<(), NameResolutionError> {
        let identifiers = descendant_identifiers(self.tree(), node);
        let token = if self.node_kind(node)? == NodeKind::DropStatement {
            identifiers.last().copied()
        } else {
            direct_identifier(self.tree(), node).or_else(|| identifiers.first().copied())
        }
        .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))?;
        let name = self.symbol(token)?;
        if let Some(binding) = self.lookup_current_callable(name) {
            self.record_use(token, binding.target)?;
            return Ok(());
        }
        let boundary = *self.callable_boundaries.last().ok_or(
            NameResolutionInternalError::InvalidSyntaxNode(self.source.block()),
        )?;
        if let Some(binding) = self.lookup_scopes_before(name, boundary) {
            if binding.target.is_callable_binding() {
                return Err(diagnostic::implicit_capture(
                    self.spelling(name)?,
                    self.origin(token)?,
                    binding.origin,
                )
                .into());
            }
            self.record_use(token, binding.target)?;
            return Ok(());
        }
        if let Some(target) = self.bindings.source_name(self.tree().source(), name) {
            self.record_use(token, NameTarget::Exported(target))?;
            return Ok(());
        }
        Err(diagnostic::unknown_name(self.spelling(name)?, self.origin(token)?).into())
    }

    /// Freezes source-backed names in a body type path before typed construction.
    ///
    /// Generic parameters, `Self`, and associated projections remain type-directed. Named
    /// builtin declarations and module segments use the same exact source namespace as other
    /// source-backed declarations.
    fn resolve_type_override(&mut self, node: NodeId) -> Result<(), NameResolutionError> {
        let tokens = self
            .tree()
            .children(node)
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Token(token)
                    if matches!(
                        token.kind(),
                        TokenKind::Identifier
                            | TokenKind::Keyword(
                                nocter_syntax::Keyword::Void | nocter_syntax::Keyword::Never
                            )
                    ) =>
                {
                    Some(*token)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let Some(first) = tokens.first().copied() else {
            // `Self` is represented by a nested `SelfType` node and remains type-directed.
            return Ok(());
        };
        let name = self.symbol(first)?;
        let mut target = if let Some(binding) = self.lookup_all_scopes(name) {
            binding.target
        } else if let Some(target) = self.bindings.source_name(self.tree().source(), name) {
            NameTarget::Exported(target)
        } else {
            return Ok(());
        };
        self.record_use(first, target)?;
        for token in tokens.into_iter().skip(1) {
            let NameTarget::Exported(ExportedEntity::Module(module)) = target else {
                break;
            };
            let selected = self.resolve_exported_module_member(module, token)?;
            target = NameTarget::Exported(selected);
            self.record_use(token, target)?;
        }
        Ok(())
    }

    fn record_use(
        &mut self,
        token: SyntaxToken,
        target: NameTarget,
    ) -> Result<(), NameResolutionInternalError> {
        self.uses
            .push(ResolvedNameUse::new(SyntaxOrigin::Token(token), target));
        self.project_use(token, target)
    }

    fn project_use(
        &mut self,
        token: SyntaxToken,
        target: NameTarget,
    ) -> Result<(), NameResolutionInternalError> {
        let entity = semantic_entity(self.source.body(), target);
        self.projections.push(Projection::new(
            entity,
            SourceRole::Reference,
            self.origin(token)?,
        ));
        Ok(())
    }

    fn pattern_introductions(
        &self,
        root: NodeId,
    ) -> Result<Vec<Introduction>, NameResolutionInternalError> {
        let Some(pattern) = descendant(self.tree(), root, NodeKind::EnumPattern) else {
            return Ok(Vec::new());
        };
        let mut result = Vec::new();
        for slot in outermost_descendants(self.tree(), pattern, NodeKind::PayloadSlot) {
            let token = direct_identifier(self.tree(), slot)
                .ok_or(NameResolutionInternalError::InvalidSyntaxNode(slot))?;
            if self.spelling(token_symbol(
                self.input.sources(),
                self.graph.symbols(),
                token,
            )?)? == "_"
            {
                continue;
            }
            result.push(Introduction {
                token,
                kind: LocalBindingKind::PatternPayload,
                documentation: None,
            });
        }
        Ok(result)
    }

    fn capture_mode(&self, node: NodeId) -> Result<CaptureMode, NameResolutionInternalError> {
        self.tree()
            .children(node)
            .iter()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => match token.kind() {
                    TokenKind::Punctuation(Punctuation::Ampersand) => Some(CaptureMode::Readonly),
                    TokenKind::Punctuation(Punctuation::ReadWrite) => Some(CaptureMode::ReadWrite),
                    TokenKind::Keyword(Keyword::Move) => Some(CaptureMode::Move),
                    _ => None,
                },
                SyntaxElement::Node(_) | SyntaxElement::Missing(_) => None,
            })
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))
    }

    fn lookup_current_callable(&self, name: Symbol) -> Option<ActiveBinding> {
        let boundary = *self.callable_boundaries.last()?;
        self.active[boundary..]
            .iter()
            .rev()
            .find_map(|scope| scope.names.get(&name).copied())
    }

    fn lookup_all_scopes(&self, name: Symbol) -> Option<ActiveBinding> {
        self.active
            .iter()
            .rev()
            .find_map(|scope| scope.names.get(&name).copied())
    }

    fn lookup_scopes_before(&self, name: Symbol, end: usize) -> Option<ActiveBinding> {
        self.active[..end]
            .iter()
            .rev()
            .find_map(|scope| scope.names.get(&name).copied())
    }

    fn push_scope(&mut self, parent: Option<BodyScopeId>) -> BodyScopeId {
        let id = self.scopes.insert(BodyScope::new(parent));
        self.active.push(ActiveScope {
            id,
            names: BTreeMap::new(),
        });
        id
    }

    fn record_block_scope(
        &mut self,
        block: NodeId,
        scope: BodyScopeId,
    ) -> Result<(), NameResolutionInternalError> {
        if self.block_scopes.insert(block, scope).is_some() {
            return Err(NameResolutionInternalError::InvalidSyntaxNode(block));
        }
        self.projections.push(Projection::new(
            SemanticEntity::BodyScope(self.source.body(), scope),
            SourceRole::Implementation,
            SourceOrigin::from_node(self.tree(), block).map_err(|_| {
                NameResolutionInternalError::InvalidSyntaxOrigin(SyntaxOrigin::Node(block))
            })?,
        ));
        Ok(())
    }

    fn current_names_mut(
        &mut self,
    ) -> Result<&mut BTreeMap<Symbol, ActiveBinding>, NameResolutionInternalError> {
        self.active.last_mut().map(|scope| &mut scope.names).ok_or(
            NameResolutionInternalError::InvalidSyntaxNode(self.source.block()),
        )
    }

    fn parameter_origin(
        &self,
        parameter: ParameterId,
    ) -> Result<SourceOrigin, NameResolutionInternalError> {
        let declarations = self.bindings.parameter_declarations(parameter);
        let token = declarations
            .iter()
            .find(|token| token.source() == self.tree().source())
            .or_else(|| declarations.first())
            .copied()
            .ok_or(NameResolutionInternalError::MissingParameterProjection(
                parameter,
            ))?;
        let tree = self
            .input
            .modules()
            .iter()
            .flat_map(nocter_compile_input::ModuleInput::sources)
            .map(nocter_compile_input::ModuleSourceInput::syntax)
            .find(|tree| tree.source() == token.source())
            .ok_or(NameResolutionInternalError::InvalidSyntaxOrigin(
                SyntaxOrigin::Token(token),
            ))?;
        SourceOrigin::from_token(tree, token).map_err(|_| {
            NameResolutionInternalError::InvalidSyntaxOrigin(SyntaxOrigin::Token(token))
        })
    }

    fn symbol(&self, token: SyntaxToken) -> Result<Symbol, NameResolutionInternalError> {
        token_symbol(self.input.sources(), self.graph.symbols(), token)
    }

    fn spelling(&self, symbol: Symbol) -> Result<&str, NameResolutionInternalError> {
        self.graph
            .symbols()
            .spelling(symbol)
            .ok_or_else(|| NameResolutionInternalError::MissingSymbol(format!("{symbol:?}").into()))
    }

    fn origin(&self, token: SyntaxToken) -> Result<SourceOrigin, NameResolutionInternalError> {
        SourceOrigin::from_token(self.tree(), token).map_err(|_| {
            NameResolutionInternalError::InvalidSyntaxOrigin(SyntaxOrigin::Token(token))
        })
    }

    fn tree(&self) -> &'syntax nocter_syntax::SyntaxTree {
        self.source.syntax()
    }

    fn node_kind(&self, node: NodeId) -> Result<NodeKind, NameResolutionInternalError> {
        self.tree()
            .node(node)
            .map(nocter_syntax::SyntaxNode::kind)
            .ok_or(NameResolutionInternalError::InvalidSyntaxNode(node))
    }
}

const fn semantic_entity(body: nocter_model::BodyId, target: NameTarget) -> SemanticEntity {
    match target {
        NameTarget::Parameter(id) => SemanticEntity::Parameter(id),
        NameTarget::Local(id) => SemanticEntity::LocalBinding(body, id),
        NameTarget::Capture(id) => SemanticEntity::Capture(body, id),
        NameTarget::Exported(entity) => match entity {
            ExportedEntity::BuiltinType(builtin) => SemanticEntity::BuiltinType(builtin),
            ExportedEntity::Module(id) => SemanticEntity::Module(id),
            ExportedEntity::NominalType(id) => SemanticEntity::NominalType(id),
            ExportedEntity::TypeAlias(id) => SemanticEntity::TypeAlias(id),
            ExportedEntity::Interface(id) => SemanticEntity::Interface(id),
            ExportedEntity::Constant(id) => SemanticEntity::Constant(id),
            ExportedEntity::Callable(id) => SemanticEntity::Callable(id),
        },
    }
}

fn descendant(
    tree: &nocter_syntax::SyntaxTree,
    root: NodeId,
    expected: NodeKind,
) -> Option<NodeId> {
    outermost_descendants(tree, root, expected)
        .into_iter()
        .next()
}

const fn node_for_error(source: BodySource<'_>) -> NodeId {
    source.block()
}
