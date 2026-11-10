//! Iterative physical serialization. A repeated array adds one cursor, never N pending entries.
use super::Failure;
use crate::{MachineLayoutKind, MachineLayoutPlan, MachineTarget};
use nocter_model::{FrozenData, FrozenNode, TypeId};
use nocter_runtime_contract::{RuntimeEnvironment, RuntimeTypeRepresentation};

enum Work<'a> {
    Value(TypeId, usize, u64),
    Array(Array<'a>),
}
struct Array<'a> {
    element: TypeId,
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
    fn next(&mut self) -> Result<Option<(TypeId, usize, u64)>, Failure> {
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
    ty: TypeId,
    value: &FrozenData,
    bytes: &mut [u8],
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
            FrozenNode::Scalar(value) => {
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
