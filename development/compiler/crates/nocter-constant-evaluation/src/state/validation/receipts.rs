//! A component receipt changes only its selected subtree, never unrelated initialized data.
use super::{INVALID, Program, Result, Type};
use crate::state::Projection;

pub(in crate::state) fn compatible_receipt(
    program: &Program,
    mut before: usize,
    mut after: usize,
    path: &[Projection],
) -> Result<()> {
    for first in path {
        if before == after {
            return Ok(());
        }
        let (left, right) = match (program.types.get(before), program.types.get(after), first) {
            (Some(Type::Record(left)), Some(Type::Record(right)), Projection::Field(selected))
                if left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .enumerate()
                        .all(|(index, (left, right))| index == *selected || left == right) =>
            {
                (
                    *left.get(*selected).ok_or(INVALID)?,
                    *right.get(*selected).ok_or(INVALID)?,
                )
            }
            (
                Some(Type::CapturedFunction {
                    code: left,
                    environment: a,
                }),
                Some(Type::CapturedFunction {
                    code: right,
                    environment: b,
                }),
                Projection::Field(0),
            ) if left == right => (*a, *b),
            (
                Some(Type::Variant {
                    primary: a,
                    alternate: left,
                }),
                Some(Type::Variant {
                    primary: b,
                    alternate: right,
                }),
                Projection::Field(0),
            ) if left == right => (*a, *b),
            (
                Some(Type::Variant {
                    primary: left,
                    alternate: Some(a),
                }),
                Some(Type::Variant {
                    primary: right,
                    alternate: Some(b),
                }),
                Projection::Field(1),
            ) if left == right => (*a, *b),
            (
                Some(Type::Array {
                    element: a,
                    length: left,
                }),
                Some(Type::Array {
                    element: b,
                    length: right,
                }),
                projection,
            ) if left == right && (*left == 1 || a == b) => {
                match projection {
                    Projection::Field(index)
                        if u64::try_from(*index).is_ok_and(|index| index < *left) => {}
                    Projection::Index { bound, .. } if *left != 0 && bound == left => {}
                    _ => return Err(INVALID),
                }
                (*a, *b)
            }
            _ => return Err(INVALID),
        };
        before = left;
        after = right;
    }
    Ok(())
}
