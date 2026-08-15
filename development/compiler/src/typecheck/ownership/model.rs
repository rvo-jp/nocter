use super::*;

#[derive(Debug, Clone)]
pub(super) struct ActiveBorrow {
    pub(super) source: BorrowPlace,
    pub(super) borrow_name: String,
    pub(super) borrow_span: ByteSpan,
    pub(super) is_readwrite: bool,
    /// Keeps compiler-created loans alive for the full nested scope.
    pub(super) scope_bound: bool,
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
    pub(super) root: crate::resolve::LocalSymbolId,
    pub(super) root_name: String,
    pub(super) fields: Option<Vec<String>>,
}

impl BorrowPlace {
    pub(super) fn whole(root: crate::resolve::LocalSymbolId, root_name: String) -> Self {
        Self {
            root,
            root_name,
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
            return self.root_name.clone();
        };
        if fields.is_empty() {
            self.root_name.clone()
        } else {
            format!("{}.{}", self.root_name, fields.join("."))
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
    places: PlaceStateForest,
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
            self.define_binding(name.to_string(), span, ty, resolved, environment);
        } else {
            if let Some(symbol) = resolved.local_symbol_id_at_span(span) {
                self.places.remove_root(symbol);
            }
        }
    }

    pub(super) fn ensure_binding_from_environment(
        &mut self,
        name: &str,
        span: ByteSpan,
        environment: &TypeEnvironment,
        resolved: &ResolveOutput,
    ) {
        let Some(symbol) = resolved.local_symbol_id_at_span(span) else {
            return;
        };
        if self.places.contains_root(symbol) {
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
        environment: &TypeEnvironment,
    ) {
        let Some(symbol) = resolved.local_symbol_id_at_span(span) else {
            return;
        };
        if non_copy_owned_type_kind_in_environment(ty, resolved, environment).is_some()
            || matches!(ty, Type::Parameter(name) if !environment
                .generic_requirements(name)
                .is_some_and(|requirements| requirements.has_copy()))
        {
            if self.places.contains_root(symbol) {
                self.places
                    .initialize(&BorrowPlace::whole(symbol, name), span);
            } else {
                self.places
                    .define_root(symbol, PlaceState::Initialized { span });
            }
        } else {
            self.places.remove_root(symbol);
        }
    }

    pub(super) fn require_initialized(
        &self,
        sources: &SourceMap,
        identifier: &IdentifierExpr,
        resolved: &ResolveOutput,
        action: &'static str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let Some(symbol) = resolved.local_symbol_id_for_reference_span(identifier.span) else {
            return true;
        };
        let place = BorrowPlace::whole(symbol, identifier.name.clone());
        self.require_place_initialized(sources, &place, identifier.span, action, diagnostics)
    }

    fn require_place_initialized(
        &self,
        sources: &SourceMap,
        place: &BorrowPlace,
        span: ByteSpan,
        action: &'static str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        let Some(state) = self.places.state(place) else {
            return true;
        };
        if !state.is_initialized() {
            diagnostics.push(uninitialized_binding_diagnostic(
                sources,
                &place.display(),
                span,
                action,
                state.previous_action(),
                state.previous_span(),
            ));
            return false;
        }
        true
    }

    pub(super) fn join_branches(&mut self, branch_ownerships: &[OwnershipState]) {
        if branch_ownerships.is_empty() {
            return;
        }
        let branches = branch_ownerships
            .iter()
            .map(|ownership| ownership.places.clone())
            .collect::<Vec<_>>();
        self.places.join_from(&branches);
    }

    pub(super) fn move_binding(
        &mut self,
        sources: &SourceMap,
        identifier: &IdentifierExpr,
        resolved: &ResolveOutput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !self.require_initialized(sources, identifier, resolved, "move", diagnostics) {
            return;
        }
        let Some(symbol) = resolved.local_symbol_id_for_reference_span(identifier.span) else {
            return;
        };
        self.places.invalidate(
            &BorrowPlace::whole(symbol, identifier.name.clone()),
            PlaceState::Moved {
                span: identifier.span,
            },
        );
    }

    pub(super) fn move_place(
        &mut self,
        sources: &SourceMap,
        place: BorrowPlace,
        span: ByteSpan,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !self.require_place_initialized(sources, &place, span, "move", diagnostics) {
            return;
        }
        self.places.invalidate(&place, PlaceState::Moved { span });
    }

    pub(super) fn drop_binding(
        &mut self,
        sources: &SourceMap,
        name: &str,
        span: ByteSpan,
        resolved: &ResolveOutput,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let identifier = IdentifierExpr {
            span,
            name: name.to_string(),
        };
        if !self.require_initialized(sources, &identifier, resolved, "drop", diagnostics) {
            return;
        }
        let Some(symbol) = resolved.local_symbol_id_at_span(span) else {
            return;
        };
        self.places.invalidate(
            &BorrowPlace::whole(symbol, name.to_string()),
            PlaceState::Dropped { span },
        );
    }
}
