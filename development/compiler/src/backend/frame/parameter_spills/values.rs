use super::*;

pub(super) fn record_i32_value_parameter_spill_requests(
    value: &I32Value,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        I32Value::Const(_) => {}
        I32Value::Location(I32Location::Parameter(index)) => {
            requests.insert(*index);
        }
        I32Value::Location(I32Location::Return | I32Location::Local(_)) => {}
        I32Value::U8ZeroExtend(value) => {
            record_u8_value_parameter_spill_requests(value, requests);
        }
        I32Value::IntegerWord(value) => {
            record_usize_value_parameter_spill_requests(value, requests);
        }
        I32Value::SliceIndex { source, index } => {
            record_slice_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
    }
}

pub(super) fn record_u8_value_parameter_spill_requests(
    value: &U8Value,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        U8Value::Const(_) => {}
        U8Value::Location(U8Location::Parameter(index)) => {
            requests.insert(*index);
        }
        U8Value::Location(U8Location::Return | U8Location::Local(_)) => {}
        U8Value::StrIndex { source, index } => {
            record_str_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
        U8Value::StaticStrIndex { index, .. } => {
            record_usize_value_parameter_spill_requests(index, requests);
        }
        U8Value::SliceIndex { source, index } => {
            record_slice_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
    }
}

pub(super) fn record_usize_value_parameter_spill_requests(
    value: &UsizeValue,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        UsizeValue::Const(_)
        | UsizeValue::ProcessArgCount
        | UsizeValue::ProcessEnvironmentCount
        | UsizeValue::CurrentAllocationState
        | UsizeValue::CurrentAllocationKind => {}
        UsizeValue::Location(UsizeLocation::Parameter(index)) => {
            requests.insert(*index);
        }
        UsizeValue::Location(UsizeLocation::Return | UsizeLocation::Local(_)) => {}
        UsizeValue::U8ZeroExtend(value) => {
            record_u8_value_parameter_spill_requests(value, requests);
        }
        UsizeValue::I32SignExtend(value) => {
            record_i32_value_parameter_spill_requests(value, requests);
        }
        UsizeValue::SliceIndex { source, index }
        | UsizeValue::IntegerSliceIndex { source, index, .. } => {
            record_slice_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
        UsizeValue::StrPointer(StrLocation::Parameter(index))
        | UsizeValue::SlicePointer(SliceLocation::Parameter(index)) => {
            requests.insert(*index);
        }
        UsizeValue::StrLen(StrLocation::Parameter(index))
        | UsizeValue::SliceLen(SliceLocation::Parameter(index)) => {
            if let Some(len_index) = index.checked_add(1) {
                requests.insert(len_index);
            }
        }
        UsizeValue::StrPointer(StrLocation::Return | StrLocation::Local(_))
        | UsizeValue::SlicePointer(SliceLocation::Return | SliceLocation::Local(_))
        | UsizeValue::StrLen(StrLocation::Return | StrLocation::Local(_))
        | UsizeValue::SliceLen(SliceLocation::Return | SliceLocation::Local(_)) => {}
    }
}

pub(super) fn record_bool_value_parameter_spill_requests(
    value: &BoolValue,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        BoolValue::Const(_) => {}
        BoolValue::Location(BoolLocation::Parameter(index)) => {
            requests.insert(*index);
        }
        BoolValue::Location(BoolLocation::Return | BoolLocation::Local(_)) => {}
        BoolValue::SliceIndex { source, index } => {
            record_slice_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
        BoolValue::Not(value) => {
            record_bool_value_parameter_spill_requests(value, requests);
        }
        BoolValue::Logical { left, right, .. } | BoolValue::BoolComparison { left, right, .. } => {
            record_bool_value_parameter_spill_requests(left, requests);
            record_bool_value_parameter_spill_requests(right, requests);
        }
        BoolValue::I32Comparison { left, right, .. } => {
            record_i32_value_parameter_spill_requests(left, requests);
            record_i32_value_parameter_spill_requests(right, requests);
        }
        BoolValue::UsizeComparison { left, right, .. } => {
            record_usize_value_parameter_spill_requests(left, requests);
            record_usize_value_parameter_spill_requests(right, requests);
        }
        BoolValue::IntegerComparison { left, right, .. } => {
            record_usize_value_parameter_spill_requests(left, requests);
            record_usize_value_parameter_spill_requests(right, requests);
        }
        BoolValue::StrComparison { left, right, .. } => {
            record_str_value_parameter_spill_requests(left, requests);
            record_str_value_parameter_spill_requests(right, requests);
        }
    }
}

pub(super) fn record_str_value_parameter_spill_requests(
    value: &StrValue,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        StrValue::StaticBytes(_) => {}
        StrValue::Location(location) => {
            record_str_location_parameter_pair_spill_requests(*location, requests);
        }
        StrValue::SliceIndex { source, index } => {
            record_slice_location_parameter_pair_spill_requests(*source, requests);
            record_usize_value_parameter_spill_requests(index, requests);
        }
        StrValue::ProcessArg { index }
        | StrValue::ProcessEnvironmentName { index }
        | StrValue::ProcessEnvironmentValue { index } => {
            record_usize_value_parameter_spill_requests(index, requests);
        }
    }
}

pub(super) fn record_slice_value_parameter_spill_requests(
    value: &SliceValue,
    requests: &mut BTreeSet<usize>,
) {
    match value {
        SliceValue::StrBytes(text) => {
            record_str_value_parameter_spill_requests(text, requests);
        }
        SliceValue::Location(location) => {
            record_slice_location_parameter_pair_spill_requests(*location, requests);
        }
    }
}

pub(super) fn record_str_location_parameter_pair_spill_requests(
    location: StrLocation,
    requests: &mut BTreeSet<usize>,
) {
    if let StrLocation::Parameter(index) = location {
        requests.insert(index);
        if let Some(len_index) = index.checked_add(1) {
            requests.insert(len_index);
        }
    }
}

pub(super) fn record_slice_location_parameter_pair_spill_requests(
    location: SliceLocation,
    requests: &mut BTreeSet<usize>,
) {
    if let SliceLocation::Parameter(index) = location {
        requests.insert(index);
        if let Some(len_index) = index.checked_add(1) {
            requests.insert(len_index);
        }
    }
}

pub(super) fn record_aggregate_location_parameter_spill_request(
    location: AggregateLocation,
    offset: u32,
    size: u64,
    requests: &mut BTreeSet<usize>,
) {
    match location {
        AggregateLocation::Parameter(index) => {
            requests.insert(index);
        }
        AggregateLocation::DirectParameter { start_index } => {
            let offset = u64::from(offset);
            let Some(last_byte_offset) = size
                .checked_sub(1)
                .and_then(|last| offset.checked_add(last))
            else {
                return;
            };
            let first_word = offset / 8;
            let last_word = last_byte_offset / 8;
            for word in first_word..=last_word {
                if let Some(parameter_index) = usize::try_from(word)
                    .ok()
                    .and_then(|word| start_index.checked_add(word))
                {
                    requests.insert(parameter_index);
                }
            }
        }
        AggregateLocation::Return
        | AggregateLocation::DirectReturn
        | AggregateLocation::Slot(_) => {}
    }
}

pub(super) fn record_borrow_source_parameter_spill_request(
    source: BorrowSource,
    requests: &mut BTreeSet<usize>,
) {
    match source {
        BorrowSource::I32(I32Location::Parameter(index))
        | BorrowSource::U8(U8Location::Parameter(index))
        | BorrowSource::Usize(UsizeLocation::Parameter(index))
        | BorrowSource::Bool(BoolLocation::Parameter(index)) => {
            requests.insert(index);
        }
        BorrowSource::BorrowParameter(index)
        | BorrowSource::AggregateParameter(index)
        | BorrowSource::AggregateParameterField {
            parameter_index: index,
            ..
        } => {
            requests.insert(index);
        }
        BorrowSource::SliceIndex { source, index, .. } => {
            record_slice_location_parameter_pair_spill_requests(source, requests);
            record_slice_element_index_parameter_spill_request(index, requests);
        }
        BorrowSource::AggregateIndex { source, index, .. } => {
            if let AggregateLocation::Parameter(index) = source {
                requests.insert(index);
            }
            record_slice_element_index_parameter_spill_request(index, requests);
        }
        BorrowSource::PointerOffset {
            pointer, offset, ..
        } => {
            if let UsizeLocation::Parameter(index) = pointer {
                requests.insert(index);
            }
            if let UsizeLocation::Parameter(index) = offset {
                requests.insert(index);
            }
        }
        BorrowSource::I32(I32Location::Return | I32Location::Local(_))
        | BorrowSource::U8(U8Location::Return | U8Location::Local(_))
        | BorrowSource::Usize(UsizeLocation::Return | UsizeLocation::Local(_))
        | BorrowSource::Bool(BoolLocation::Return | BoolLocation::Local(_))
        | BorrowSource::AggregateSlot(_)
        | BorrowSource::AggregateSlotField { .. } => {}
        BorrowSource::BorrowLocal(UsizeLocation::Parameter(index)) => {
            requests.insert(index);
        }
        BorrowSource::BorrowLocal(UsizeLocation::Return | UsizeLocation::Local(_)) => {}
    }
}

pub(super) fn record_slice_element_index_parameter_spill_request(
    index: SliceElementIndex,
    requests: &mut BTreeSet<usize>,
) {
    if let SliceElementIndex::Location(UsizeLocation::Parameter(index)) = index {
        requests.insert(index);
    }
}
