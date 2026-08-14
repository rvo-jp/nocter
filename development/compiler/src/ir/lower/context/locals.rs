use super::*;

impl<'a> LoweringContext<'a> {
    pub(in crate::ir::lower) fn next_i32_local_location(
        &self,
    ) -> Result<I32Location, Vec<Diagnostic>> {
        self.next_local_index(1).map(I32Location::Local)
    }

    pub(in crate::ir::lower) fn next_u8_local_location(
        &self,
    ) -> Result<U8Location, Vec<Diagnostic>> {
        self.next_local_index(1).map(U8Location::Local)
    }

    pub(in crate::ir::lower) fn next_usize_local_location(
        &self,
    ) -> Result<UsizeLocation, Vec<Diagnostic>> {
        self.next_local_index(1).map(UsizeLocation::Local)
    }

    pub(in crate::ir::lower) fn first_temporary_local_index(
        &self,
    ) -> Result<usize, Vec<Diagnostic>> {
        Ok(self.used_local_abi_words())
    }

    pub(in crate::ir::lower) fn next_bool_local_location(
        &self,
    ) -> Result<BoolLocation, Vec<Diagnostic>> {
        self.next_local_index(1).map(BoolLocation::Local)
    }

    pub(in crate::ir::lower) fn next_str_local_location(
        &self,
    ) -> Result<StrLocation, Vec<Diagnostic>> {
        self.next_local_index(2).map(StrLocation::Local)
    }

    pub(in crate::ir::lower) fn next_slice_local_location(
        &self,
    ) -> Result<SliceLocation, Vec<Diagnostic>> {
        self.next_local_index(2).map(SliceLocation::Local)
    }

    pub(in crate::ir::lower) fn with_reserved_local_abi_words(&self, words: usize) -> Self {
        let mut context = self.clone();
        context.reserved_local_abi_words += words;
        context
    }

    pub(in crate::ir::lower) fn with_reserved_error_local_abi_words(&self) -> Self {
        self.with_reserved_local_abi_words(LocalKind::Error.abi_word_count())
    }

    pub(in crate::ir::lower) fn define_i32_local(&mut self, name: String) {
        self.define_local(name, LocalKind::I32);
    }

    pub(in crate::ir::lower) fn define_u8_local(&mut self, name: String) {
        self.define_local(name, LocalKind::U8);
    }

    pub(in crate::ir::lower) fn define_usize_local(&mut self, name: String) {
        self.define_local(name, LocalKind::Usize);
    }

    pub(in crate::ir::lower) fn define_integer_local(&mut self, name: String, kind: IntegerType) {
        self.define_local(name, LocalKind::Integer(kind));
    }

    pub(in crate::ir::lower) fn define_borrow_local(
        &mut self,
        name: String,
        is_readwrite: bool,
        inner: Type,
    ) {
        self.define_local(
            name,
            LocalKind::Borrow {
                is_readwrite,
                inner,
            },
        );
    }

    pub(in crate::ir::lower) fn define_aggregate_borrow_local(
        &mut self,
        name: String,
        is_readwrite: bool,
        inner: Type,
        fields: Vec<AggregateField>,
    ) {
        self.aggregate_local_borrow_fields
            .insert(name.clone(), fields);
        self.define_borrow_local(name, is_readwrite, inner);
    }

    pub(in crate::ir::lower) fn reserve_drop_state_usize_local(
        &mut self,
    ) -> Result<UsizeLocation, Vec<Diagnostic>> {
        let index = self.next_local_index(1)?;
        self.locals.push(LocalBinding {
            name: format!("<drop-state-{index}>"),
            kind: LocalKind::Usize,
            index,
        });
        Ok(UsizeLocation::Local(index))
    }

    pub(in crate::ir::lower) fn reserve_drop_state_bool_local(
        &mut self,
    ) -> Result<BoolLocation, Vec<Diagnostic>> {
        let index = self.next_local_index(1)?;
        self.locals.push(LocalBinding {
            name: format!("<drop-state-{index}>"),
            kind: LocalKind::Bool,
            index,
        });
        Ok(BoolLocation::Local(index))
    }

    pub(in crate::ir::lower) fn define_bool_local(&mut self, name: String) {
        self.define_local(name, LocalKind::Bool);
    }

    pub(in crate::ir::lower) fn define_str_local(&mut self, name: String) {
        self.define_local(name, LocalKind::Str);
    }

    pub(in crate::ir::lower) fn define_slice_local(
        &mut self,
        name: String,
        element_kind: TypecheckSliceElementKind,
        element_type: Option<TypeExpr>,
    ) {
        self.define_local(
            name,
            LocalKind::Slice(SliceTypeInfo {
                element_kind,
                element_type,
            }),
        );
    }

    pub(in crate::ir::lower) fn rename_local(&mut self, old_name: &str, new_name: String) -> bool {
        let Some(local) = self.locals.iter_mut().find(|local| local.name == old_name) else {
            return false;
        };
        local.name = new_name;
        true
    }

    pub(in crate::ir::lower) fn define_aggregate_local(
        &mut self,
        name: String,
        layout: ValueLayout,
        is_copy: bool,
        drop_kind: Option<AggregateDrop>,
        fields: Vec<AggregateField>,
    ) -> usize {
        let slot_index = self.reserve_aggregate_slot_index();
        self.locals.push(LocalBinding {
            name,
            kind: LocalKind::Aggregate {
                layout,
                slot_index,
                is_copy,
                drop_obligation: DropObligation::for_drop_kind(&drop_kind),
                drop_kind,
                runtime_live: None,
            },
            index: 0,
        });
        self.aggregate_fields.insert(slot_index, fields);
        slot_index
    }

    pub(in crate::ir::lower) fn define_error_local(
        &mut self,
        name: String,
    ) -> Result<(StrLocation, StrLocation), Vec<Diagnostic>> {
        let index = self.next_local_index(LocalKind::Error.abi_word_count())?;
        self.locals.push(LocalBinding {
            name,
            kind: LocalKind::Error,
            index,
        });
        Ok((StrLocation::Local(index), StrLocation::Local(index + 2)))
    }

    pub(in crate::ir::lower) fn next_error_local_locations(
        &self,
    ) -> Result<(StrLocation, StrLocation), Vec<Diagnostic>> {
        let index = self.next_local_index(LocalKind::Error.abi_word_count())?;
        Ok((StrLocation::Local(index), StrLocation::Local(index + 2)))
    }

    pub(in crate::ir::lower) fn error_local_locations(
        &self,
        name: &str,
    ) -> Option<(StrLocation, StrLocation)> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Error)
            .map(|local| {
                (
                    StrLocation::Local(local.index),
                    StrLocation::Local(local.index + 2),
                )
            })
    }

    pub(in crate::ir::lower) fn i32_location(&self, name: &str) -> Option<I32Location> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::I32)
            .map(|local| I32Location::Local(local.index))
            .or_else(|| {
                self.i32_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(I32Location::Parameter)
            })
    }

    pub(in crate::ir::lower) fn usize_location(&self, name: &str) -> Option<UsizeLocation> {
        self.locals
            .iter()
            .find(|local| {
                local.name == name && matches!(local.kind, LocalKind::Usize | LocalKind::Integer(_))
            })
            .map(|local| UsizeLocation::Local(local.index))
            .or_else(|| {
                self.usize_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(UsizeLocation::Parameter)
            })
            .or_else(|| {
                self.integer_parameters
                    .iter()
                    .position(|parameter| {
                        parameter
                            .as_ref()
                            .is_some_and(|(parameter_name, _)| parameter_name == name)
                    })
                    .map(UsizeLocation::Parameter)
            })
    }

    pub(in crate::ir::lower) fn integer_kind(&self, name: &str) -> Option<IntegerType> {
        self.locals
            .iter()
            .find_map(|local| match &local.kind {
                LocalKind::Integer(kind) if local.name == name => Some(*kind),
                _ => None,
            })
            .or_else(|| {
                self.integer_parameters.iter().find_map(|parameter| {
                    let (parameter_name, kind) = parameter.as_ref()?;
                    (parameter_name == name).then_some(*kind)
                })
            })
    }

    pub(in crate::ir::lower) fn borrow_local(
        &self,
        name: &str,
    ) -> Option<(UsizeLocation, bool, &Type)> {
        self.locals.iter().find_map(|local| {
            let LocalKind::Borrow {
                is_readwrite,
                inner,
            } = &local.kind
            else {
                return None;
            };
            (local.name == name).then_some((
                UsizeLocation::Local(local.index),
                *is_readwrite,
                inner,
            ))
        })
    }

    pub(in crate::ir::lower) fn u8_location(&self, name: &str) -> Option<U8Location> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::U8)
            .map(|local| U8Location::Local(local.index))
            .or_else(|| {
                self.u8_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(U8Location::Parameter)
            })
    }

    pub(in crate::ir::lower) fn bool_location(&self, name: &str) -> Option<BoolLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Bool)
            .map(|local| BoolLocation::Local(local.index))
            .or_else(|| {
                self.bool_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(BoolLocation::Parameter)
            })
    }

    pub(in crate::ir::lower) fn str_location(&self, name: &str) -> Option<StrLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Str)
            .map(|local| StrLocation::Local(local.index))
            .or_else(|| {
                self.str_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(StrLocation::Parameter)
            })
    }

    pub(in crate::ir::lower) fn slice_location(&self, name: &str) -> Option<SliceLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && matches!(local.kind, LocalKind::Slice(_)))
            .map(|local| SliceLocation::Local(local.index))
            .or_else(|| {
                self.slice_parameters
                    .iter()
                    .position(|parameter| {
                        parameter
                            .as_ref()
                            .is_some_and(|parameter| parameter.name == name)
                    })
                    .map(SliceLocation::Parameter)
            })
    }

    pub(in crate::ir::lower) fn slice_element_kind(
        &self,
        name: &str,
    ) -> Option<TypecheckSliceElementKind> {
        self.locals
            .iter()
            .find_map(|local| match &local.kind {
                LocalKind::Slice(info) if local.name == name => Some(info.element_kind),
                _ => None,
            })
            .or_else(|| {
                self.slice_parameters
                    .iter()
                    .find_map(|parameter| match parameter {
                        Some(parameter) if parameter.name == name => {
                            Some(parameter.info.element_kind)
                        }
                        _ => None,
                    })
            })
    }

    pub(in crate::ir::lower) fn slice_element_type_expr(&self, name: &str) -> Option<&TypeExpr> {
        self.locals
            .iter()
            .find_map(|local| match &local.kind {
                LocalKind::Slice(info) if local.name == name => info.element_type.as_ref(),
                _ => None,
            })
            .or_else(|| {
                self.slice_parameters
                    .iter()
                    .find_map(|parameter| match parameter {
                        Some(parameter) if parameter.name == name => {
                            parameter.info.element_type.as_ref()
                        }
                        _ => None,
                    })
            })
    }

    pub(in crate::ir::lower) fn error_code_location(&self, name: &str) -> Option<StrLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Error)
            .map(|local| StrLocation::Local(local.index))
            .or_else(|| {
                self.error_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(StrLocation::Parameter)
            })
    }

    pub(in crate::ir::lower) fn error_message_location(&self, name: &str) -> Option<StrLocation> {
        self.locals
            .iter()
            .find(|local| local.name == name && local.kind == LocalKind::Error)
            .map(|local| StrLocation::Local(local.index + 2))
            .or_else(|| {
                self.error_parameters
                    .iter()
                    .position(|parameter| parameter.as_deref() == Some(name))
                    .map(|index| StrLocation::Parameter(index + 2))
            })
    }

    pub(in crate::ir::lower) fn aggregate_slot(&self, name: &str) -> Option<(usize, ValueLayout)> {
        self.aggregate_local(name)
            .map(|local| (local.slot_index, local.layout))
    }

    pub(in crate::ir::lower) fn aggregate_local(&self, name: &str) -> Option<AggregateLocal> {
        self.locals.iter().find_map(|local| {
            if local.name == name
                && let LocalKind::Aggregate {
                    layout,
                    slot_index,
                    is_copy,
                    ref drop_kind,
                    runtime_live,
                    ..
                } = local.kind
            {
                return Some(AggregateLocal {
                    slot_index,
                    layout,
                    is_copy,
                    drop_kind: drop_kind.clone(),
                    runtime_live,
                });
            }
            None
        })
    }

    pub(in crate::ir::lower) fn aggregate_local_by_slot(
        &self,
        slot_index: usize,
    ) -> Option<AggregateLocal> {
        self.locals.iter().find_map(|local| {
            let LocalKind::Aggregate {
                layout,
                slot_index: local_slot_index,
                is_copy,
                ref drop_kind,
                runtime_live,
                ..
            } = local.kind
            else {
                return None;
            };
            if local_slot_index == slot_index {
                return Some(AggregateLocal {
                    slot_index: local_slot_index,
                    layout,
                    is_copy,
                    drop_kind: drop_kind.clone(),
                    runtime_live,
                });
            }
            None
        })
    }

    pub(in crate::ir::lower) fn aggregate_local_fields(
        &self,
        name: &str,
    ) -> Option<Vec<AggregateField>> {
        let local = self.aggregate_local(name)?;
        self.aggregate_fields.get(&local.slot_index).cloned()
    }

    pub(in crate::ir::lower) fn mark_aggregate_local_dropped(&mut self, name: &str) {
        if let Some(local) = self.locals.iter_mut().find(|local| local.name == name) {
            match &mut local.kind {
                LocalKind::Aggregate {
                    drop_obligation, ..
                } => {
                    *drop_obligation = DropObligation::Inactive;
                }
                LocalKind::Outcome(outcome) => {
                    outcome.drop_obligation = DropObligation::Inactive;
                    outcome.is_live = false;
                }
                _ => {}
            }
        }
    }

    pub(in crate::ir::lower) fn mark_aggregate_local_dropped_by_slot(&mut self, slot_index: usize) {
        let Some(local) = self.locals.iter_mut().find(|local| {
            matches!(
                &local.kind,
                LocalKind::Aggregate {
                    slot_index: local_slot_index,
                    ..
                } if *local_slot_index == slot_index
            )
        }) else {
            return;
        };
        let LocalKind::Aggregate {
            drop_obligation, ..
        } = &mut local.kind
        else {
            return;
        };
        *drop_obligation = DropObligation::Inactive;
    }

    pub(in crate::ir::lower) fn mark_aggregate_local_moved(&mut self, name: &str) {
        if self
            .aggregate_local(name)
            .is_some_and(|local| local.runtime_live.is_some())
        {
            return;
        }
        self.update_aggregate_drop_obligation(name, DropObligation::Inactive);
    }

    pub(in crate::ir::lower) fn mark_aggregate_local_initialized(&mut self, name: &str) {
        let Some(local) = self
            .locals
            .iter_mut()
            .find(|local| local.name == name && matches!(local.kind, LocalKind::Aggregate { .. }))
        else {
            return;
        };
        let LocalKind::Aggregate {
            drop_kind,
            drop_obligation,
            ..
        } = &mut local.kind
        else {
            return;
        };
        *drop_obligation = DropObligation::for_drop_kind(drop_kind);
    }

    pub(in crate::ir::lower) fn mark_aggregate_local_array_prefix(
        &mut self,
        name: &str,
        initialized: UsizeLocation,
        elements: Vec<ArrayElementDropState>,
    ) -> bool {
        let Some(local) = self
            .locals
            .iter_mut()
            .find(|local| local.name == name && matches!(local.kind, LocalKind::Aggregate { .. }))
        else {
            return false;
        };
        let LocalKind::Aggregate {
            drop_kind,
            drop_obligation,
            ..
        } = &mut local.kind
        else {
            return false;
        };
        if !matches!(drop_kind, Some(AggregateDrop::Array(_))) {
            return false;
        }
        *drop_obligation = DropObligation::ArrayPrefix {
            initialized,
            elements,
        };
        true
    }

    pub(in crate::ir::lower) fn mark_aggregate_local_struct_fields(
        &mut self,
        name: &str,
        fields: Vec<StructFieldDropState>,
    ) -> bool {
        let Some(local) = self
            .locals
            .iter_mut()
            .find(|local| local.name == name && matches!(local.kind, LocalKind::Aggregate { .. }))
        else {
            return false;
        };
        let LocalKind::Aggregate {
            drop_kind,
            drop_obligation,
            ..
        } = &mut local.kind
        else {
            return false;
        };
        if !matches!(
            drop_kind,
            Some(AggregateDrop::Direct(_) | AggregateDrop::Struct(_))
        ) {
            return false;
        }
        *drop_obligation = DropObligation::StructFields { fields };
        true
    }

    pub(in crate::ir::lower) fn mark_aggregate_local_payload_fields(
        &mut self,
        name: &str,
        tag: u8,
        fields: Vec<PayloadFieldDropState>,
    ) -> bool {
        let Some(local) = self
            .locals
            .iter_mut()
            .find(|local| local.name == name && matches!(local.kind, LocalKind::Aggregate { .. }))
        else {
            return false;
        };
        let LocalKind::Aggregate {
            drop_kind,
            drop_obligation,
            ..
        } = &mut local.kind
        else {
            return false;
        };
        if !matches!(drop_kind, Some(AggregateDrop::PayloadEnum(_))) {
            return false;
        }
        *drop_obligation = DropObligation::PayloadFields { tag, fields };
        true
    }

    pub(in crate::ir::lower) fn pending_aggregate_drops(&self) -> Vec<PendingAggregateDrop> {
        let mut pending = self.pending_temporary_aggregate_drops();
        pending.extend(
            self.locals
                .iter()
                .rev()
                .filter_map(|local| {
                    let (layout, slot_index, drop_obligation, drop_kind, runtime_live) =
                        match &local.kind {
                            LocalKind::Aggregate {
                                layout,
                                slot_index,
                                drop_obligation,
                                drop_kind,
                                runtime_live,
                                ..
                            } => (
                                *layout,
                                *slot_index,
                                drop_obligation,
                                drop_kind,
                                *runtime_live,
                            ),
                            LocalKind::Outcome(outcome) if outcome.is_live => (
                                outcome.storage.layout,
                                outcome.slot_index,
                                &outcome.drop_obligation,
                                &outcome.drop_kind,
                                None,
                            ),
                            _ => return None,
                        };
                    if !drop_obligation.is_active() {
                        return None;
                    }
                    Some(PendingAggregateDrop {
                        name: local.name.clone(),
                        slot_index,
                        layout,
                        drop_kind: drop_kind.clone()?,
                        obligation: drop_obligation.clone(),
                        runtime_live,
                    })
                })
                .collect::<Vec<_>>(),
        );
        pending
    }

    pub(in crate::ir::lower) fn local_mark(&self) -> usize {
        self.locals.len()
    }

    pub(in crate::ir::lower) fn aggregate_local_defined_since(
        &self,
        name: &str,
        local_mark: usize,
    ) -> bool {
        self.locals
            .get(local_mark..)
            .unwrap_or(&[])
            .iter()
            .any(|local| local.name == name && matches!(local.kind, LocalKind::Aggregate { .. }))
    }

    pub(in crate::ir::lower) fn local_defined_since(&self, name: &str, local_mark: usize) -> bool {
        self.locals
            .get(local_mark..)
            .unwrap_or(&[])
            .iter()
            .any(|local| local.name == name)
    }

    pub(in crate::ir::lower) fn pending_aggregate_drops_since(
        &self,
        local_mark: usize,
    ) -> Vec<PendingAggregateDrop> {
        let locals = self.locals.get(local_mark..).unwrap_or(&[]);
        let mut pending = self.pending_temporary_aggregate_drops();
        pending.extend(
            locals
                .iter()
                .rev()
                .filter_map(|local| {
                    let (layout, slot_index, drop_obligation, drop_kind, runtime_live) =
                        match &local.kind {
                            LocalKind::Aggregate {
                                layout,
                                slot_index,
                                drop_obligation,
                                drop_kind,
                                runtime_live,
                                ..
                            } => (
                                *layout,
                                *slot_index,
                                drop_obligation,
                                drop_kind,
                                *runtime_live,
                            ),
                            LocalKind::Outcome(outcome) if outcome.is_live => (
                                outcome.storage.layout,
                                outcome.slot_index,
                                &outcome.drop_obligation,
                                &outcome.drop_kind,
                                None,
                            ),
                            _ => return None,
                        };
                    if !drop_obligation.is_active() {
                        return None;
                    }
                    Some(PendingAggregateDrop {
                        name: local.name.clone(),
                        slot_index,
                        layout,
                        drop_kind: drop_kind.clone()?,
                        obligation: drop_obligation.clone(),
                        runtime_live,
                    })
                })
                .collect::<Vec<_>>(),
        );
        pending
    }

    pub(in crate::ir::lower) fn pending_aggregate_drop_by_slot(
        &self,
        slot_index: usize,
    ) -> Option<PendingAggregateDrop> {
        self.locals.iter().find_map(|local| {
            let LocalKind::Aggregate {
                layout,
                slot_index: local_slot_index,
                ref drop_obligation,
                ref drop_kind,
                runtime_live,
                ..
            } = local.kind
            else {
                return None;
            };
            if local_slot_index != slot_index || !drop_obligation.is_active() {
                return None;
            }
            Some(PendingAggregateDrop {
                name: local.name.clone(),
                slot_index,
                layout,
                drop_kind: drop_kind.clone()?,
                obligation: drop_obligation.clone(),
                runtime_live,
            })
        })
    }

    pub(in crate::ir::lower) fn promote_aggregate_runtime_live(
        &mut self,
        name: &str,
    ) -> Result<Option<Instruction>, Vec<Diagnostic>> {
        let initially_live = self.aggregate_local(name).is_some_and(|local| {
            self.pending_aggregate_drop_by_slot(local.slot_index)
                .is_some()
        });
        self.track_aggregate_runtime_live(name, initially_live)
    }

    pub(in crate::ir::lower) fn track_uninitialized_aggregate_local(
        &mut self,
        name: &str,
    ) -> Result<Option<Instruction>, Vec<Diagnostic>> {
        self.track_aggregate_runtime_live(name, false)
    }

    fn track_aggregate_runtime_live(
        &mut self,
        name: &str,
        initially_live: bool,
    ) -> Result<Option<Instruction>, Vec<Diagnostic>> {
        let Some(local_index) = self.locals.iter().position(|local| {
            local.name == name
                && matches!(
                    local.kind,
                    LocalKind::Aggregate {
                        is_copy: false,
                        drop_kind: Some(_),
                        ..
                    }
                )
        }) else {
            return Ok(None);
        };
        let existing = match &self.locals[local_index].kind {
            LocalKind::Aggregate { runtime_live, .. } => *runtime_live,
            _ => unreachable!("aggregate runtime state selection must remain aggregate"),
        };
        if existing.is_some() {
            return Ok(None);
        }
        let destination = self.reserve_drop_state_bool_local()?;
        let LocalKind::Aggregate { runtime_live, .. } = &mut self.locals[local_index].kind else {
            unreachable!("aggregate runtime state selection must remain aggregate");
        };
        *runtime_live = Some(destination);
        Ok(Some(Instruction::SetBool {
            destination,
            value: BoolValue::Const(initially_live),
        }))
    }

    pub(in crate::ir::lower) fn aggregate_runtime_live_by_slot(
        &self,
        slot_index: usize,
    ) -> Option<BoolLocation> {
        self.locals.iter().find_map(|local| match local.kind {
            LocalKind::Aggregate {
                slot_index: local_slot,
                runtime_live,
                ..
            } if local_slot == slot_index => runtime_live,
            _ => None,
        })
    }

    pub(in crate::ir::lower) fn aggregate_runtime_live_transition(
        &self,
        name: &str,
        is_live: bool,
    ) -> Option<Instruction> {
        let runtime_live = self.aggregate_local(name)?.runtime_live?;
        Some(Instruction::SetBool {
            destination: runtime_live,
            value: BoolValue::Const(is_live),
        })
    }

    pub(in crate::ir::lower) fn aggregate_runtime_drop_candidate_names(&self) -> Vec<String> {
        self.locals
            .iter()
            .filter_map(|local| match &local.kind {
                LocalKind::Aggregate {
                    is_copy: false,
                    drop_kind: Some(_),
                    ..
                } => Some(local.name.clone()),
                _ => None,
            })
            .collect()
    }

    pub(in crate::ir::lower) fn aggregate_field(
        &self,
        aggregate_name: &str,
        field_name: &str,
    ) -> Option<AggregateFieldAccess> {
        self.aggregate_local_field(aggregate_name, field_name)
            .or_else(|| self.aggregate_borrow_field(aggregate_name, field_name))
            .or_else(|| self.aggregate_local_borrow_field(aggregate_name, field_name))
    }

    pub(in crate::ir::lower) fn closure_capture_field(
        &self,
        name: &str,
    ) -> Option<&AggregateFieldAccess> {
        self.closure_capture_fields.get(name)
    }

    fn aggregate_local_field(
        &self,
        aggregate_name: &str,
        field_name: &str,
    ) -> Option<AggregateFieldAccess> {
        let aggregate = self.aggregate_local(aggregate_name)?;
        self.aggregate_fields
            .get(&aggregate.slot_index)?
            .iter()
            .find(|field| field.name == field_name)
            .map(|field| AggregateFieldAccess {
                source: AggregateLocation::Slot(aggregate.slot_index),
                offset: field.offset,
                kind: field.kind.clone(),
                is_readwrite: true,
                is_copy: field.is_copy,
                drop_kind: field.drop_kind.clone(),
            })
    }

    fn aggregate_borrow_field(
        &self,
        aggregate_name: &str,
        field_name: &str,
    ) -> Option<AggregateFieldAccess> {
        let borrow = self
            .aggregate_borrows
            .iter()
            .find(|borrow| borrow.name == aggregate_name)?;
        borrow
            .fields
            .iter()
            .find(|field| field.name == field_name)
            .map(|field| AggregateFieldAccess {
                source: AggregateLocation::Parameter(borrow.parameter_index),
                offset: field.offset,
                kind: field.kind.clone(),
                is_readwrite: borrow.is_readwrite,
                is_copy: field.is_copy,
                drop_kind: field.drop_kind.clone(),
            })
    }

    fn aggregate_local_borrow_field(
        &self,
        aggregate_name: &str,
        field_name: &str,
    ) -> Option<AggregateFieldAccess> {
        let (location, is_readwrite, _) = self.borrow_local(aggregate_name)?;
        self.aggregate_local_borrow_fields
            .get(aggregate_name)?
            .iter()
            .find(|field| field.name == field_name)
            .map(|field| AggregateFieldAccess {
                source: AggregateLocation::Borrow(location),
                offset: field.offset,
                kind: field.kind.clone(),
                is_readwrite,
                is_copy: field.is_copy,
                drop_kind: field.drop_kind.clone(),
            })
    }

    pub(in crate::ir::lower) fn aggregate_borrow_parameter(
        &self,
        aggregate_name: &str,
    ) -> Option<&AggregateBorrowParameter> {
        self.aggregate_borrows
            .iter()
            .find(|borrow| borrow.name == aggregate_name)
    }

    pub(in crate::ir::lower) fn borrow_parameter(&self, name: &str) -> Option<&BorrowParameter> {
        self.borrow_parameters
            .iter()
            .find(|borrow| borrow.name == name)
    }

    fn next_local_index(&self, required_words: usize) -> Result<usize, Vec<Diagnostic>> {
        let index = self.used_local_abi_words();
        index.checked_add(required_words).ok_or_else(|| {
            vec![Diagnostic::error(
                "E8008",
                "local ABI word count overflows host usize",
            )]
        })?;

        Ok(index)
    }

    fn define_local(&mut self, name: String, kind: LocalKind) {
        let index = self.used_local_abi_words();
        self.locals.push(LocalBinding { name, kind, index });
    }

    fn update_aggregate_drop_obligation(&mut self, name: &str, obligation: DropObligation) {
        let Some(local) = self
            .locals
            .iter_mut()
            .find(|local| local.name == name && matches!(local.kind, LocalKind::Aggregate { .. }))
        else {
            return;
        };
        let LocalKind::Aggregate {
            drop_obligation, ..
        } = &mut local.kind
        else {
            return;
        };
        *drop_obligation = obligation;
    }

    fn used_local_abi_words(&self) -> usize {
        self.reserved_local_abi_words
            + self
                .locals
                .iter()
                .map(|local| local.kind.abi_word_count())
                .sum::<usize>()
    }

    pub(in crate::ir::lower) fn reserve_aggregate_slot_index(&self) -> usize {
        let slot_index = self.next_aggregate_slot_index.get();
        self.next_aggregate_slot_index.set(slot_index + 1);
        slot_index
    }

    pub(in crate::ir::lower) fn aggregate_slot_mark(&self) -> usize {
        self.next_aggregate_slot_index.get()
    }

    pub(in crate::ir::lower) fn restore_aggregate_slot_mark(&self, mark: usize) {
        self.next_aggregate_slot_index.set(mark);
    }

    pub(in crate::ir::lower) fn aggregate_slot_counter(&self) -> Rc<Cell<usize>> {
        self.next_aggregate_slot_index.clone()
    }
}
