//! Select a tag and shared payload storage for a value-model enum.
use super::{
    LayoutBuilder, MachineLayout, MachineLayoutError, MachineLayoutKind, MachinePayloadLayout,
    TaggedGeometry,
};
use nocter_model::{AccessFieldId, VariantId};
use nocter_runtime_contract::{RuntimeDataId, RuntimePrimitive, RuntimeValueVariantRepresentation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineValueEnumVariantLayout {
    variant: VariantId,
    fields: Box<[(AccessFieldId, MachinePayloadLayout)]>,
}

impl MachineValueEnumVariantLayout {
    #[must_use]
    pub const fn variant(&self) -> VariantId {
        self.variant
    }

    #[must_use]
    pub const fn fields(&self) -> &[(AccessFieldId, MachinePayloadLayout)] {
        &self.fields
    }
}

impl LayoutBuilder<'_> {
    pub(super) fn value_enumeration(
        &mut self,
        ty: RuntimeDataId,
        variants: &[RuntimeValueVariantRepresentation],
    ) -> Result<MachineLayout, MachineLayoutError> {
        let primitive = RuntimePrimitive::unsigned_tag(variants.len())
            .ok_or(MachineLayoutError::InvalidRepresentation(ty))?;
        let tag_layout = self.primitive(ty, primitive)?;
        let MachineLayoutKind::Scalar(tag) = tag_layout.kind else {
            return Err(MachineLayoutError::InvalidRepresentation(ty));
        };
        self.require_layouts(
            variants
                .iter()
                .flat_map(|variant| variant.fields().iter().map(|&(_, ty)| ty)),
        )?;
        let TaggedGeometry {
            size,
            alignment,
            payload_offset,
            relative,
        } = self.tagged_geometry(
            ty,
            (tag_layout.size, tag_layout.alignment),
            variants
                .iter()
                .map(|variant| variant.fields().iter().map(|&(_, ty)| ty)),
        )?;
        let variants = variants
            .iter()
            .zip(relative)
            .map(|(variant, offsets)| {
                let fields = variant
                    .fields()
                    .iter()
                    .zip(offsets)
                    .map(|(&(field, data), offset)| {
                        let offset = payload_offset
                            .checked_add(offset)
                            .ok_or(MachineLayoutError::LayoutOverflow(ty))?;
                        Ok((field, MachinePayloadLayout { ty: data, offset }))
                    })
                    .collect::<Result<_, _>>()?;
                Ok(MachineValueEnumVariantLayout {
                    variant: variant.variant(),
                    fields,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(MachineLayout {
            size,
            alignment,
            kind: MachineLayoutKind::ValueEnum {
                tag,
                tag_offset: 0,
                payload_offset,
                variants,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MachineLayoutPlan, MachineScalar};
    use nocter_model::ArenaBuilder;
    use nocter_runtime_contract::{
        RuntimeAbiIdentity, RuntimeEnvironment, RuntimeType, RuntimeTypeRepresentation,
        RuntimeTypeRepresentationTable, RuntimeTypeTableBuilder,
    };
    use std::collections::BTreeMap;

    #[test]
    fn tagged_enum_layout_retains_field_identity_and_tag_width() {
        let enum_data = RuntimeDataId::Instance(0);
        let small = RuntimeDataId::Instance(1);
        let wide = RuntimeDataId::Instance(2);
        let mut fields = ArenaBuilder::<AccessFieldId, ()>::new();
        let first_member = fields.insert(());
        let small_member = fields.insert(());
        let wide_member = fields.insert(());
        let first_payload = [(first_member, small)];
        let mut variants = ArenaBuilder::<VariantId, ()>::new();

        for (count, bits, members, payload_base, size, alignment) in [
            (
                256,
                8,
                &[(small_member, small), (wide_member, wide)][..],
                8,
                24,
                8,
            ),
            (
                257,
                16,
                &[(small_member, small), (wide_member, wide)][..],
                8,
                24,
                8,
            ),
            (257, 16, &[(small_member, small)][..], 2, 4, 2),
        ] {
            let variants: Vec<_> = (0..count)
                .map(|index| {
                    let fields = if index == 0 {
                        &first_payload[..]
                    } else if index + 1 == count {
                        members
                    } else {
                        &[]
                    };
                    RuntimeValueVariantRepresentation::new(variants.insert(()), fields)
                })
                .collect();
            let last = variants.last().unwrap().variant();
            let mut types = RuntimeTypeTableBuilder::new();
            types.insert(enum_data, RuntimeType::Aggregate).unwrap();
            types
                .insert(small, RuntimeType::Primitive(RuntimePrimitive::Unsigned(8)))
                .unwrap();
            types
                .insert(wide, RuntimeType::Primitive(RuntimePrimitive::Unsigned(64)))
                .unwrap();
            let runtime = RuntimeEnvironment::new(
                types.finish().unwrap(),
                RuntimeTypeRepresentationTable::new(BTreeMap::from([(
                    enum_data,
                    RuntimeTypeRepresentation::ValueEnum {
                        variants: variants.into(),
                    },
                )])),
                RuntimeAbiIdentity::Arm64DarwinV1,
            )
            .unwrap();
            let layouts = MachineLayoutPlan::from_runtime(&runtime, [enum_data]).unwrap();
            let layout = layouts.get(enum_data).unwrap();
            assert_eq!((layout.size(), layout.alignment()), (size, alignment));
            let MachineLayoutKind::ValueEnum {
                tag,
                tag_offset,
                payload_offset,
                variants,
            } = layout.kind()
            else {
                panic!("tagged enum layout");
            };
            assert_eq!(
                *tag,
                MachineScalar::Integer {
                    bits,
                    signed: false
                }
            );
            assert_eq!((*tag_offset, *payload_offset), (0, payload_base));
            assert_eq!(variants[0].fields()[0].0, first_member);
            assert_eq!(variants[0].fields()[0].1.offset(), payload_base);
            assert!(variants[1].fields().is_empty());
            let last_case = variants.last().unwrap();
            assert_eq!(last_case.variant(), last);
            assert_eq!(last_case.fields()[0].0, small_member);
            assert_eq!(last_case.fields()[0].1.offset(), payload_base);
            if let Some(&(field, layout)) = last_case.fields().get(1) {
                assert_eq!(
                    (field, layout.ty(), layout.offset()),
                    (wide_member, wide, 16)
                );
            }
        }
    }
}
