//! Physical compatibility of already paired canonical carrier schemas; no source/ownership proof.
use super::{MachineLayoutKind as Kind, MachineLayoutPlan};
use nocter_runtime_contract::RuntimeDataId as Data;
use std::collections::BTreeSet;

impl MachineLayoutPlan {
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
