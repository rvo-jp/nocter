//! Physical compatibility of already paired canonical carrier schemas; no source/ownership proof.
use super::{MachineLayoutKind as Kind, MachineLayoutPlan};
use nocter_model::{ValuePath, ValueProjection};
use nocter_runtime_contract::RuntimeDataId as Data;
use std::collections::BTreeSet;

impl MachineLayoutPlan {
    /// Compare relative offsets for corresponding logical projections. None denotes any
    /// existing array coordinate; no active tag, index value/bounds, common root address,
    /// backing capacity, source correspondence or relocation permission is established.
    #[must_use]
    pub fn has_same_projection_offset(
        &self,
        mut before: Data,
        mut after: Data,
        path: &ValuePath<Option<u64>>,
    ) -> bool {
        if self.get(before).is_none() || self.get(after).is_none() {
            return false;
        }
        let (mut old_offset, mut new_offset) = (0u64, 0u64);
        for projection in path.fields() {
            if let ValueProjection::Array(None) = projection {
                let (Some(left), Some(right)) = (self.get(before), self.get(after)) else {
                    return false;
                };
                let (
                    Kind::FixedArray {
                        element: old,
                        length: old_length,
                        stride: old_stride,
                    },
                    Kind::FixedArray {
                        element: new,
                        length: new_length,
                        stride: new_stride,
                    },
                ) = (left.kind(), right.kind())
                else {
                    return false;
                };
                if old_length != new_length
                    || *old_length == 0
                    || *old_length > 1 && old_stride != new_stride
                {
                    return false;
                }
                before = *old;
                after = *new;
                continue;
            }
            let Ok(field) = projection.try_map_index(|index| index.ok_or(())) else {
                return false;
            };
            let (Some(old), Some(new)) = (
                self.value_field(before, field),
                self.value_field(after, field),
            ) else {
                return false;
            };
            let (Some(left), Some(right)) = (
                old_offset.checked_add(old.offset()),
                new_offset.checked_add(new.offset()),
            ) else {
                return false;
            };
            old_offset = left;
            new_offset = right;
            before = old.ty();
            after = new.ty();
        }
        self.get(before).is_some() && self.get(after).is_some() && old_offset == new_offset
    }

    /// Compare complete corresponding stored geometry, including nested member offsets and ABI
    /// classes. Callers must establish source-schema correspondence independently. This permits no
    /// relocation, initialized value, live reference or source operation. Missing layouts reject.
    #[must_use]
    pub fn has_same_geometry(&self, before: Data, after: Data) -> bool {
        let mut pending = vec![(before, after)];
        let mut seen = BTreeSet::new();
        while let Some((before, after)) = pending.pop() {
            if !seen.insert((before, after)) {
                continue;
            }
            let (Some(left), Some(right)) = (self.get(before), self.get(after)) else {
                return false;
            };
            if left.size() != right.size()
                || left.alignment() != right.alignment()
                || !matches!((self.class(before), self.class(after)), (Some(a), Some(b)) if a == b)
            {
                return false;
            }
            if before != after && !same_kind(left.kind(), right.kind(), &mut pending) {
                return false;
            }
        }
        true
    }
}
fn optional(left: Option<Data>, right: Option<Data>, pending: &mut Vec<(Data, Data)>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            pending.push((left, right));
            true
        }
        _ => false,
    }
}
fn members(
    left: impl ExactSizeIterator<Item = (u64, Data)>,
    right: impl ExactSizeIterator<Item = (u64, Data)>,
    pending: &mut Vec<(Data, Data)>,
) -> bool {
    if left.len() != right.len() {
        return false;
    }
    for ((a, left), (b, right)) in left.zip(right) {
        if a != b {
            return false;
        }
        pending.push((left, right));
    }
    true
}
#[allow(clippy::too_many_lines)]
fn same_kind(left: &Kind, right: &Kind, pending: &mut Vec<(Data, Data)>) -> bool {
    match (left, right) {
        (Kind::Unit, Kind::Unit)
        | (Kind::Pointer, Kind::Pointer)
        | (Kind::ErrorHandle, Kind::ErrorHandle) => true,
        (Kind::Scalar(a), Kind::Scalar(b)) => a == b,
        (Kind::StaticCallable { environment: a }, Kind::StaticCallable { environment: b }) => {
            optional(*a, *b, pending)
        }
        (
            Kind::CallableAlternatives {
                tag: a,
                payload_offset: ap,
                cases: ac,
            },
            Kind::CallableAlternatives {
                tag: b,
                payload_offset: bp,
                cases: bc,
            },
        ) => {
            a == b
                && ap == bp
                && members(
                    ac.iter().map(|case| (*ap, *case)),
                    bc.iter().map(|case| (*bp, *case)),
                    pending,
                )
        }
        (Kind::Opaque { witness: a }, Kind::Opaque { witness: b }) => {
            pending.push((*a, *b));
            true
        }
        (
            Kind::View {
                pointer_offset: ap,
                length_offset: al,
            },
            Kind::View {
                pointer_offset: bp,
                length_offset: bl,
            },
        ) => ap == bp && al == bl,
        (Kind::RuntimeStorage { role: a }, Kind::RuntimeStorage { role: b }) => a == b,
        (Kind::Struct { fields: a }, Kind::Struct { fields: b }) => members(
            a.iter().map(|field| (field.offset(), field.ty())),
            b.iter().map(|field| (field.offset(), field.ty())),
            pending,
        ),
        (Kind::Tuple { elements: a }, Kind::Tuple { elements: b }) => members(
            a.iter().map(|field| (field.offset(), field.ty())),
            b.iter().map(|field| (field.offset(), field.ty())),
            pending,
        ),
        (Kind::Closure { captures: a }, Kind::Closure { captures: b }) => members(
            a.iter().map(|field| (field.offset(), field.ty())),
            b.iter().map(|field| (field.offset(), field.ty())),
            pending,
        ),
        (
            Kind::FixedArray {
                element: a,
                length: al,
                stride: astride,
            },
            Kind::FixedArray {
                element: b,
                length: bl,
                stride: bstride,
            },
        ) => {
            if al != bl || astride != bstride {
                return false;
            }
            if *al != 0 {
                pending.push((*a, *b));
            }
            true
        }
        (
            Kind::Enum {
                tag_offset: at,
                payload_offset: ap,
                variants: a,
            },
            Kind::Enum {
                tag_offset: bt,
                payload_offset: bp,
                variants: b,
            },
        ) => {
            at == bt
                && ap == bp
                && a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| {
                    a.tag() == b.tag()
                        && members(
                            a.payload().iter().map(|field| (field.offset(), field.ty())),
                            b.payload().iter().map(|field| (field.offset(), field.ty())),
                            pending,
                        )
                })
        }
        (
            Kind::Outcome {
                kind: ak,
                tag_offset: at,
                payload_offset: ap,
                primary: a,
                alternate: aa,
            },
            Kind::Outcome {
                kind: bk,
                tag_offset: bt,
                payload_offset: bp,
                primary: b,
                alternate: ba,
            },
        ) => {
            ak == bk
                && at == bt
                && ap == bp
                && optional(*a, *b, pending)
                && optional(*aa, *ba, pending)
        }
        (Kind::PackEntry { key: ak, value: av }, Kind::PackEntry { key: bk, value: bv }) => {
            members(
                [*ak, *av]
                    .into_iter()
                    .map(|field| (field.offset(), field.ty())),
                [*bk, *bv]
                    .into_iter()
                    .map(|field| (field.offset(), field.ty())),
                pending,
            )
        }
        (
            Kind::ErasedCallable {
                environment_offset: ae,
                invoke_offset: ai,
                destroy_offset: ad,
                allocation_size_offset: aa,
            },
            Kind::ErasedCallable {
                environment_offset: be,
                invoke_offset: bi,
                destroy_offset: bd,
                allocation_size_offset: ba,
            },
        ) => ae == be && ai == bi && ad == bd && aa == ba,
        _ => false,
    }
}
