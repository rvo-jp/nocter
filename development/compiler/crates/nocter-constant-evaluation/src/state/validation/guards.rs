//! Validate sparse owner guards without inventing initialized backing.
use super::{INVALID, Place, Program, Result, Storage, View, bounded_array, place};
use crate::state::{Missing, ProjectionPath};
pub(super) fn validate(
    program: &Program,
    plan: &View<'_>,
    alternative: &crate::state::Alternative,
) -> Result<()> {
    for (&slot, paths) in &alternative.owned {
        if super::definition(plan, slot)?.storage != Storage::Owned {
            return Err(INVALID);
        }
        if paths.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(INVALID);
        }
        let mut previous_value: Option<&ProjectionPath> = None;
        let mut previous_prefix = None;
        for missing in paths {
            if missing.indices().any(|(slot, _)| {
                alternative
                    .owned
                    .get(&slot)
                    .is_none_or(|holes| !holes.is_empty())
            }) {
                return Err(INVALID);
            }
            match missing {
                Missing::Value(fields) => {
                    if fields.is_empty()
                        || previous_value.is_some_and(|previous| fields.starts_with(previous))
                    {
                        return Err(INVALID);
                    }
                    previous_value = Some(fields);
                    place(
                        program,
                        plan,
                        &Place {
                            slot,
                            fields: fields.clone(),
                        },
                    )?;
                }
                Missing::ArrayPrefix {
                    base,
                    endpoint,
                    bound,
                } => {
                    if previous_prefix == Some(base)
                        || bounded_array(
                            program,
                            plan,
                            &Place {
                                slot,
                                fields: base.clone(),
                            },
                            *endpoint,
                        )? != *bound
                    {
                        return Err(INVALID);
                    }
                    previous_prefix = Some(base);
                }
            }
        }
    }
    Ok(())
}
