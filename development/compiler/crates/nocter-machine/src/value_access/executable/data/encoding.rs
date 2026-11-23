//! Iterative physical serialization. A repeated array adds one cursor, never N pending entries.
use super::Failure;
use crate::{MachineLayoutKind, MachineLayoutPlan, MachineTarget};
use nocter_model::{FrozenData, FrozenNode};
use nocter_runtime_contract::RuntimeDataId;
use nocter_runtime_contract::{RuntimeEnvironment, RuntimeTypeRepresentation};

enum Work<'a> {
    Value(RuntimeDataId, usize, u64),
    Array(Array<'a>),
}
struct Array<'a> {
    element: RuntimeDataId,
    stride: u64,
    offset: u64,
    remaining: u64,
    node: &'a FrozenNode,
    replacements: &'a [(u64, usize)],
}
impl<'a> Array<'a> {
    fn new(layout: &MachineLayoutKind, node: &'a FrozenNode, offset: u64) -> Result<Self, Failure> {
        let MachineLayoutKind::FixedArray {
            element,
            length,
            stride,
        } = layout
        else {
            return Err(Failure::InvalidRoot);
        };
        let replacements = match node {
            FrozenNode::Aggregate(values) if u64::try_from(values.len()) == Ok(*length) => &[][..],
            FrozenNode::Repeated {
                length: count,
                replacements,
                ..
            } if count.get() == *length => replacements,
            _ => return Err(Failure::InvalidRoot),
        };
        Ok(Self {
            element: *element,
            stride: *stride,
            offset,
            remaining: *length,
            node,
            replacements,
        })
    }
    fn next(&mut self) -> Result<Option<(RuntimeDataId, usize, u64)>, Failure> {
        let Some(index) = self.remaining.checked_sub(1) else {
            return Ok(None);
        };
        self.remaining = index;
        let node = match self.node {
            FrozenNode::Aggregate(values) => *values
                .get(usize::try_from(index).map_err(|_| Failure::InvalidRoot)?)
                .ok_or(Failure::InvalidRoot)?,
            FrozenNode::Repeated { element, .. } => {
                if let Some((&(key, value), rest)) = self.replacements.split_last()
                    && key == index
                {
                    self.replacements = rest;
                    value
                } else {
                    *element
                }
            }
            _ => return Err(Failure::InvalidRoot),
        };
        let offset = index
            .checked_mul(self.stride)
            .and_then(|part| self.offset.checked_add(part))
            .ok_or(Failure::InvalidRoot)?;
        Ok(Some((self.element, node, offset)))
    }
}

pub(super) fn write(
    runtime: &RuntimeEnvironment,
    layouts: &MachineLayoutPlan,
    ty: RuntimeDataId,
    value: &FrozenData,
    bytes: &mut [u8],
    mut text_pointer: impl FnMut(u64, &str) -> Result<(), Failure>,
) -> Result<(), Failure> {
    let target = MachineTarget::select(runtime.abi());
    let mut pending = vec![Work::Value(ty, value.root(), 0)];
    while let Some(work) = pending.pop() {
        let (ty, node, offset) = match work {
            Work::Value(ty, node, offset) => (ty, node, offset),
            Work::Array(mut array) => {
                if let Some((ty, node, offset)) = array.next()? {
                    pending.push(Work::Array(array));
                    pending.push(Work::Value(ty, node, offset));
                }
                continue;
            }
        };
        let layout = layouts.get(ty).ok_or(Failure::InvalidRoot)?;
        if layout.size() == 0 {
            continue;
        }
        match &value.nodes()[node] {
            FrozenNode::EnumTag(tag) => {
                if runtime.scalar_enum_variant(ty, *tag).is_none() {
                    return Err(Failure::InvalidRoot);
                }
                crate::data::write_integer_bytes(
                    bytes,
                    offset,
                    (*tag).into(),
                    layout.size(),
                    target,
                )
                .ok_or(Failure::InvalidRoot)?;
            }
            FrozenNode::Variant { primary, payload } => {
                if let Some(payload) =
                    write_outcome(bytes, target, layout.kind(), offset, *primary, *payload)?
                {
                    pending.push(payload);
                }
            }
            FrozenNode::Scalar(value) => {
                if let nocter_model::ConstantValue::Text(text) = value {
                    let pointer =
                        crate::data::text_descriptor(bytes, offset, text, layout.kind(), target)
                            .ok_or(Failure::InvalidRoot)?;
                    text_pointer(pointer, text)?;
                    continue;
                }
                let bits = crate::data::scalar_bits(value).ok_or(Failure::InvalidRoot)?;
                crate::data::write_integer_bytes(bytes, offset, bits, layout.size(), target)
                    .ok_or(Failure::InvalidRoot)?;
            }
            node @ (FrozenNode::Aggregate(_) | FrozenNode::Repeated { .. })
                if matches!(layout.kind(), MachineLayoutKind::FixedArray { .. }) =>
            {
                pending.push(Work::Array(Array::new(layout.kind(), node, offset)?));
            }
            FrozenNode::Aggregate(values) => {
                let Some(RuntimeTypeRepresentation::ValueAggregate { fields }) =
                    runtime.type_representations().get(ty)
                else {
                    return Err(Failure::InvalidRoot);
                };
                if fields.len() != values.len() {
                    return Err(Failure::InvalidRoot);
                }
                for (field, &node) in fields.iter().zip(values).rev() {
                    let member = layouts
                        .value_field(ty, field.field())
                        .ok_or(Failure::InvalidRoot)?;
                    pending.push(Work::Value(
                        member.ty(),
                        node,
                        offset
                            .checked_add(member.offset())
                            .ok_or(Failure::InvalidRoot)?,
                    ));
                }
            }
            FrozenNode::Unit | FrozenNode::Repeated { .. } => return Err(Failure::InvalidRoot),
        }
    }
    Ok(())
}

fn write_outcome(
    bytes: &mut [u8],
    target: MachineTarget,
    layout: &MachineLayoutKind,
    offset: u64,
    primary: bool,
    payload: Option<usize>,
) -> Result<Option<Work<'static>>, Failure> {
    let MachineLayoutKind::Outcome {
        kind,
        tag_offset,
        payload_offset,
        primary: first,
        alternate,
    } = layout
    else {
        return Err(Failure::InvalidRoot);
    };
    let (tag, data) = if primary {
        (kind.primary_tag(), first)
    } else {
        (kind.alternate_tag(), alternate)
    };
    crate::data::write_integer_bytes(
        bytes,
        offset
            .checked_add(*tag_offset)
            .ok_or(Failure::InvalidRoot)?,
        tag.into(),
        1,
        target,
    )
    .ok_or(Failure::InvalidRoot)?;
    match (data, payload) {
        (Some(data), Some(node)) => Ok(Some(Work::Value(
            *data,
            node,
            offset
                .checked_add(*payload_offset)
                .ok_or(Failure::InvalidRoot)?,
        ))),
        (None, None) => Ok(None),
        _ => Err(Failure::InvalidRoot),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nocter_model::{ArenaBuilder, VariantId};
    use nocter_runtime_contract::{
        RuntimeAbiIdentity, RuntimeType, RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
    };
    use std::collections::BTreeMap;

    #[test]
    fn enum_statics_serialize_the_selected_tag_at_the_schema_width() {
        let mut ids = ArenaBuilder::<VariantId, ()>::new();
        for (count, expected) in [(256, &[255_u8][..]), (257, &[0_u8, 1][..])] {
            let data = RuntimeDataId::Instance(0);
            let variants = (0..count).map(|_| ids.insert(())).collect();
            let mut types = RuntimeTypeTableBuilder::new();
            types.insert(data, RuntimeType::Aggregate).unwrap();
            let runtime = RuntimeEnvironment::new(
                types.finish().unwrap(),
                RuntimeTypeRepresentationTable::new(BTreeMap::from([(
                    data,
                    RuntimeTypeRepresentation::ScalarEnum { variants },
                )])),
                RuntimeAbiIdentity::Arm64DarwinV1,
            )
            .unwrap();
            let layouts = MachineLayoutPlan::from_runtime(&runtime, [data]).unwrap();
            let layout = layouts.get(data).unwrap();
            assert_eq!(layout.size(), u64::try_from(expected.len()).unwrap());
            let mut bytes = vec![0; expected.len()];
            let value = FrozenData::new(vec![FrozenNode::EnumTag(count - 1)]).unwrap();
            write(
                &runtime,
                &layouts,
                data,
                &value,
                &mut bytes,
                |_, _| unreachable!(),
            )
            .unwrap();
            assert_eq!(bytes, expected);
            let invalid = FrozenData::new(vec![FrozenNode::EnumTag(count)]).unwrap();
            assert!(
                write(
                    &runtime,
                    &layouts,
                    data,
                    &invalid,
                    &mut bytes,
                    |_, _| unreachable!()
                )
                .is_err()
            );
        }
    }
}
