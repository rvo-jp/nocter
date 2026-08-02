use super::*;

impl<'a> LoweringContext<'a> {
    pub(in crate::ir::lower) fn empty(
        function_name: String,
        return_type: Type,
        function_signatures: FunctionSignatures,
    ) -> Self {
        Self {
            function_name,
            function_return_type: return_type.clone(),
            function_return_type_expr: None,
            return_type,
            function_returns_optional: false,
            function_signatures,
            call_resolution: None,
            function_names: FunctionNames::default(),
            generic_substitutions: HashMap::new(),
            literal_pack: None,
            i32_parameters: Vec::new(),
            u8_parameters: Vec::new(),
            usize_parameters: Vec::new(),
            bool_parameters: Vec::new(),
            str_parameters: Vec::new(),
            slice_parameters: Vec::new(),
            error_parameters: Vec::new(),
            reserved_local_abi_words: 0,
            locals: Vec::new(),
            aggregate_fields: HashMap::new(),
            temporary_aggregate_drops: Vec::new(),
            region_cleanups: Vec::new(),
            allocation_context_restores: Vec::new(),
            borrow_parameters: Vec::new(),
            aggregate_borrows: Vec::new(),
            error_payloads: ErrorPayloads::default(),
            next_aggregate_slot_index: Rc::new(Cell::new(0)),
        }
    }

    pub(in crate::ir::lower) fn new(
        function_name: String,
        return_type: Type,
        function_signatures: FunctionSignatures,
        parameters: LoweringParameterSlots,
    ) -> Self {
        let mut locals = Vec::new();
        let mut aggregate_fields = HashMap::new();
        let next_aggregate_slot_index = parameters
            .aggregates
            .iter()
            .map(|parameter| parameter.slot_index + 1)
            .max()
            .unwrap_or(0);
        for parameter in parameters.aggregates {
            locals.push(LocalBinding {
                name: parameter.name,
                kind: LocalKind::Aggregate {
                    layout: parameter.layout,
                    slot_index: parameter.slot_index,
                    is_copy: parameter.is_copy,
                    drop_obligation: DropObligation::for_drop_kind(&parameter.drop_kind),
                    drop_kind: parameter.drop_kind,
                },
                index: 0,
            });
            aggregate_fields.insert(parameter.slot_index, parameter.fields);
        }

        Self {
            function_name,
            function_return_type: return_type.clone(),
            function_return_type_expr: None,
            return_type,
            function_returns_optional: false,
            function_signatures,
            call_resolution: None,
            function_names: FunctionNames::default(),
            generic_substitutions: HashMap::new(),
            literal_pack: None,
            i32_parameters: parameters.i32,
            u8_parameters: parameters.u8,
            usize_parameters: parameters.usize,
            bool_parameters: parameters.bool,
            str_parameters: parameters.str,
            slice_parameters: parameters.slice,
            error_parameters: parameters.error,
            reserved_local_abi_words: 0,
            locals,
            aggregate_fields,
            temporary_aggregate_drops: Vec::new(),
            region_cleanups: Vec::new(),
            allocation_context_restores: Vec::new(),
            borrow_parameters: parameters.borrow_parameters,
            aggregate_borrows: parameters.aggregate_borrows,
            error_payloads: ErrorPayloads::default(),
            next_aggregate_slot_index: Rc::new(Cell::new(next_aggregate_slot_index)),
        }
    }

    pub(in crate::ir::lower) fn with_call_resolution(
        mut self,
        root_source: SourceId,
        resolved: &'a ResolveOutput,
        typecheck_facts: &'a TypecheckFacts,
        function_names: FunctionNames,
        resolved_sources: ResolvedSources<'a>,
    ) -> Self {
        self.call_resolution = Some(CallResolution {
            root_source,
            resolved,
            typecheck_facts,
            resolved_sources,
        });
        self.function_names = function_names;
        self
    }

    pub(in crate::ir::lower) fn with_generic_substitutions(
        mut self,
        substitutions: HashMap<String, TypeExpr>,
    ) -> Self {
        self.generic_substitutions = substitutions;
        self
    }

    pub(in crate::ir::lower) fn with_literal_pack(
        mut self,
        literal_pack: LiteralPackLowering,
    ) -> Self {
        self.literal_pack = Some(literal_pack);
        self
    }

    pub(in crate::ir::lower) fn literal_pack(&self, name: &str) -> Option<&LiteralPackLowering> {
        self.literal_pack
            .as_ref()
            .filter(|pack| pack.capture_name == name)
    }

    pub(in crate::ir::lower) fn with_function_return_type(mut self, return_type: Type) -> Self {
        self.function_return_type = return_type;
        self
    }

    pub(in crate::ir::lower) fn with_function_return_type_expr(
        mut self,
        return_type: TypeExpr,
    ) -> Self {
        self.function_return_type_expr = Some(return_type);
        self
    }

    pub(in crate::ir::lower) fn with_function_returns_optional(
        mut self,
        function_returns_optional: bool,
    ) -> Self {
        self.function_returns_optional = function_returns_optional;
        self
    }

    pub(in crate::ir::lower) fn with_error_payloads(
        mut self,
        error_payloads: ErrorPayloads,
    ) -> Self {
        self.error_payloads = error_payloads;
        self
    }
}
