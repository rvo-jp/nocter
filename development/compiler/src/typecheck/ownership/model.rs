use super::*;

#[derive(Debug, Clone)]
pub(super) struct ActiveBorrow {
    pub(super) source: BorrowPlace,
    pub(super) borrow_name: String,
    pub(super) borrow_span: ByteSpan,
    pub(super) is_readwrite: bool,
}

#[derive(Debug, Clone)]
pub(super) struct DirectBorrowSource {
    pub(super) source: BorrowPlace,
    pub(super) source_span: ByteSpan,
    pub(super) is_readwrite: bool,
}

#[derive(Debug, Clone)]
pub(super) struct BorrowAction {
    pub(super) place: BorrowPlace,
    pub(super) span: ByteSpan,
    pub(super) description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BorrowPlace {
    pub(super) root: String,
    pub(super) fields: Option<Vec<String>>,
}

impl BorrowPlace {
    pub(super) fn whole(root: String) -> Self {
        Self {
            root,
            fields: Some(Vec::new()),
        }
    }

    pub(super) fn push_field(&mut self, field: String) {
        if let Some(fields) = &mut self.fields {
            fields.push(field);
        }
    }

    pub(super) fn mark_unknown(&mut self) {
        self.fields = None;
    }

    pub(super) fn conflicts_with(&self, other: &Self) -> bool {
        if self.root != other.root {
            return false;
        }
        let (Some(left), Some(right)) = (&self.fields, &other.fields) else {
            return true;
        };
        left.starts_with(right) || right.starts_with(left)
    }

    pub(super) fn display(&self) -> String {
        let Some(fields) = &self.fields else {
            return self.root.clone();
        };
        if fields.is_empty() {
            self.root.clone()
        } else {
            format!("{}.{}", self.root, fields.join("."))
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct FlowState {
    pub(super) reaches_end: bool,
    pub(super) break_states: Vec<OwnershipState>,
    pub(super) continue_states: Vec<OwnershipState>,
}

impl FlowState {
    pub(super) fn fallthrough() -> Self {
        Self {
            reaches_end: true,
            break_states: Vec::new(),
            continue_states: Vec::new(),
        }
    }

    pub(super) fn terminal() -> Self {
        Self {
            reaches_end: false,
            break_states: Vec::new(),
            continue_states: Vec::new(),
        }
    }

    pub(super) fn break_with(state: OwnershipState) -> Self {
        Self {
            reaches_end: false,
            break_states: vec![state],
            continue_states: Vec::new(),
        }
    }

    pub(super) fn continue_with(state: OwnershipState) -> Self {
        Self {
            reaches_end: false,
            break_states: Vec::new(),
            continue_states: vec![state],
        }
    }

    pub(super) fn from_nested(flow: FlowState) -> Self {
        flow
    }

    pub(super) fn extend_nested(&mut self, flow: FlowState) {
        self.break_states.extend(flow.break_states);
        self.continue_states.extend(flow.continue_states);
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct OwnershipState {
    bindings: HashMap<String, OwnedBinding>,
}

impl OwnershipState {
    pub(super) fn define_parameters(
        &mut self,
        parameters: &[crate::ast::Parameter],
        environment: &TypeEnvironment,
        resolved: &ResolveOutput,
    ) {
        for parameter in parameters {
            self.define_binding_from_environment(
                &parameter.name,
                parameter.name_span,
                environment,
                resolved,
            );
        }
    }

    pub(super) fn define_binding_from_environment(
        &mut self,
        name: &str,
        span: ByteSpan,
        environment: &TypeEnvironment,
        resolved: &ResolveOutput,
    ) {
        if let Some(ty) = environment.get(name) {
            self.define_binding(name.to_string(), span, ty, resolved);
        } else {
            self.bindings.remove(name);
        }
    }

    pub(super) fn ensure_binding_from_environment(
        &mut self,
        name: &str,
        span: ByteSpan,
        environment: &TypeEnvironment,
        resolved: &ResolveOutput,
    ) {
        if self.bindings.contains_key(name) {
            return;
        }
        self.define_binding_from_environment(name, span, environment, resolved);
    }

    pub(super) fn define_binding(
        &mut self,
        name: String,
        span: ByteSpan,
        ty: &Type,
        resolved: &ResolveOutput,
    ) {
        if non_copy_owned_type_kind(ty, resolved).is_some() {
            self.bindings.insert(
                name,
                OwnedBinding {
                    state: BindingState::Initialized { span },
                },
            );
        } else {
            self.bindings.remove(&name);
        }
    }

    pub(super) fn require_initialized(
        &self,
        sources: &SourceMap,
        identifier: &IdentifierExpr,
        action: &'static str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let Some(binding) = self.bindings.get(&identifier.name) else {
            return true;
        };
        let BindingState::Initialized { .. } = binding.state else {
            diagnostics.push(uninitialized_binding_diagnostic(
                sources,
                &identifier.name,
                identifier.span,
                action,
                binding.state.previous_action(),
                binding.state.previous_span(),
            ));
            return false;
        };
        true
    }

    pub(super) fn join_branches(&mut self, branch_ownerships: &[OwnershipState]) {
        if branch_ownerships.is_empty() {
            return;
        }
        for (name, binding) in &mut self.bindings {
            let mut joined_state = branch_ownerships[0]
                .bindings
                .get(name)
                .map(|binding| binding.state)
                .unwrap_or(binding.state);
            for branch_ownership in &branch_ownerships[1..] {
                let branch_state = branch_ownership
                    .bindings
                    .get(name)
                    .map(|binding| binding.state)
                    .unwrap_or(binding.state);
                joined_state = BindingState::join(joined_state, branch_state);
            }
            binding.state = joined_state;
        }
    }

    pub(super) fn move_binding(
        &mut self,
        sources: &SourceMap,
        identifier: &IdentifierExpr,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !self.require_initialized(sources, identifier, "move", diagnostics) {
            return;
        }
        if let Some(binding) = self.bindings.get_mut(&identifier.name) {
            binding.state = BindingState::Moved {
                span: identifier.span,
            };
        }
    }

    pub(super) fn drop_binding(
        &mut self,
        sources: &SourceMap,
        name: &str,
        span: ByteSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let identifier = IdentifierExpr {
            span,
            name: name.to_string(),
        };
        if !self.require_initialized(sources, &identifier, "drop", diagnostics) {
            return;
        }
        if let Some(binding) = self.bindings.get_mut(name) {
            binding.state = BindingState::Dropped { span };
        }
    }
}

#[derive(Debug, Clone)]
struct OwnedBinding {
    state: BindingState,
}

#[derive(Debug, Clone, Copy)]
enum BindingState {
    Initialized { span: ByteSpan },
    Moved { span: ByteSpan },
    Dropped { span: ByteSpan },
    Uninitialized { span: ByteSpan },
    MaybeInitialized { span: ByteSpan },
}

impl BindingState {
    pub(super) fn join(left: Self, right: Self) -> Self {
        match (left, right) {
            (BindingState::Initialized { span }, BindingState::Initialized { .. }) => {
                BindingState::Initialized { span }
            }
            (BindingState::Moved { span }, BindingState::Moved { .. }) => {
                BindingState::Moved { span }
            }
            (BindingState::Dropped { span }, BindingState::Dropped { .. }) => {
                BindingState::Dropped { span }
            }
            (BindingState::Uninitialized { span }, BindingState::Uninitialized { .. }) => {
                BindingState::Uninitialized { span }
            }
            (
                BindingState::Moved { span }
                | BindingState::Dropped { span }
                | BindingState::Uninitialized { span },
                BindingState::Moved { .. }
                | BindingState::Dropped { .. }
                | BindingState::Uninitialized { .. },
            ) => BindingState::Uninitialized { span },
            (BindingState::MaybeInitialized { span }, _)
            | (_, BindingState::MaybeInitialized { span }) => {
                BindingState::MaybeInitialized { span }
            }
            (BindingState::Initialized { .. }, BindingState::Moved { span })
            | (BindingState::Moved { span }, BindingState::Initialized { .. })
            | (BindingState::Initialized { .. }, BindingState::Dropped { span })
            | (BindingState::Dropped { span }, BindingState::Initialized { .. })
            | (BindingState::Initialized { .. }, BindingState::Uninitialized { span })
            | (BindingState::Uninitialized { span }, BindingState::Initialized { .. }) => {
                BindingState::MaybeInitialized { span }
            }
        }
    }

    pub(super) fn previous_action(self) -> &'static str {
        match self {
            BindingState::Moved { .. } => "moved",
            BindingState::Dropped { .. } => "dropped",
            BindingState::Uninitialized { .. } => "uninitialized",
            BindingState::MaybeInitialized { .. } => "maybe uninitialized",
            BindingState::Initialized { .. } => "initialized",
        }
    }

    pub(super) fn previous_span(self) -> ByteSpan {
        match self {
            BindingState::Initialized { span }
            | BindingState::Moved { span }
            | BindingState::Dropped { span }
            | BindingState::Uninitialized { span }
            | BindingState::MaybeInitialized { span } => span,
        }
    }
}
