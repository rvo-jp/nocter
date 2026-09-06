use std::cmp::Ordering;

use super::{BigNat, FloatBits, FloatFormat, TargetFloatEvaluator, rounded_ratio};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatBinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatComparisonOperation {
    Equal,
    Less,
}

#[derive(Clone, Copy)]
enum Magnitude {
    Zero,
    Finite { significand: u64, exponent: i32 },
    Infinity,
    Nan,
}

#[derive(Clone, Copy)]
struct Decoded {
    negative: bool,
    magnitude: Magnitude,
}

impl FloatBits {
    #[must_use]
    pub const fn format(self) -> FloatFormat {
        match self {
            Self::Binary32(_) => FloatFormat::Binary32,
            Self::Binary64(_) => FloatFormat::Binary64,
        }
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        match self {
            Self::Binary32(bits) => bits as u64,
            Self::Binary64(bits) => bits,
        }
    }

    fn from_raw(format: FloatFormat, raw: u64) -> Self {
        match format {
            FloatFormat::Binary32 => {
                Self::Binary32(u32::try_from(raw).expect("binary32 operation cannot set high bits"))
            }
            FloatFormat::Binary64 => Self::Binary64(raw),
        }
    }
}

impl FloatFormat {
    const fn exponent_bits(self) -> u32 {
        match self {
            Self::Binary32 => 8,
            Self::Binary64 => 11,
        }
    }

    const fn total_bits(self) -> u32 {
        match self {
            Self::Binary32 => 32,
            Self::Binary64 => 64,
        }
    }

    const fn sign_mask(self) -> u64 {
        1_u64 << (self.total_bits() - 1)
    }

    const fn fraction_mask(self) -> u64 {
        (1_u64 << self.fraction_bits()) - 1
    }

    const fn exponent_mask(self) -> u64 {
        ((1_u64 << self.exponent_bits()) - 1) << self.fraction_bits()
    }

    fn infinity(self, negative: bool) -> FloatBits {
        FloatBits::from_raw(
            self,
            (if negative { self.sign_mask() } else { 0 }) | self.exponent_mask(),
        )
    }

    fn quiet_nan(self) -> FloatBits {
        FloatBits::from_raw(
            self,
            self.exponent_mask() | (1_u64 << (self.fraction_bits() - 1)),
        )
    }

    fn zero(self, negative: bool) -> FloatBits {
        FloatBits::from_raw(self, if negative { self.sign_mask() } else { 0 })
    }
}

impl TargetFloatEvaluator {
    /// Applies an IEEE sign inversion without interpreting the payload on the compiler host.
    #[must_use]
    pub fn negate(self, value: FloatBits) -> FloatBits {
        FloatBits::from_raw(value.format(), value.raw() ^ value.format().sign_mask())
    }

    /// Evaluates one matching-format IEEE arithmetic operation from raw target bits.
    #[must_use]
    pub fn binary(
        self,
        operation: FloatBinaryOperation,
        left: FloatBits,
        right: FloatBits,
    ) -> Option<FloatBits> {
        let format = left.format();
        if right.format() != format {
            return None;
        }
        let left = decode(left);
        let right = decode(right);
        Some(match operation {
            FloatBinaryOperation::Add => add(format, left, right),
            FloatBinaryOperation::Subtract => add(
                format,
                left,
                Decoded {
                    negative: !right.negative,
                    magnitude: right.magnitude,
                },
            ),
            FloatBinaryOperation::Multiply => multiply(format, left, right),
            FloatBinaryOperation::Divide => divide(format, left, right),
            FloatBinaryOperation::Remainder => remainder(format, left, right),
        })
    }

    /// Evaluates ordinary IEEE equality or strict comparison from raw target bits.
    #[must_use]
    pub fn compare(
        self,
        operation: FloatComparisonOperation,
        left: FloatBits,
        right: FloatBits,
    ) -> Option<bool> {
        if left.format() != right.format() {
            return None;
        }
        let left = decode(left);
        let right = decode(right);
        if matches!(left.magnitude, Magnitude::Nan) || matches!(right.magnitude, Magnitude::Nan) {
            return Some(false);
        }
        Some(match operation {
            FloatComparisonOperation::Equal => equal(left, right),
            FloatComparisonOperation::Less => less(left, right),
        })
    }

    /// Converts an integer exactly or with IEEE nearest-even rounding into one target format.
    #[must_use]
    pub fn integer(self, value: i128, format: FloatFormat) -> FloatBits {
        let negative = value.is_negative();
        let magnitude = BigNat::from_u128(value.unsigned_abs());
        round(format, negative, &magnitude, &BigNat::one(), 0)
    }

    /// Widens a binary32 value to its exact binary64 representation.
    #[must_use]
    pub fn widen_binary32(self, value: u32) -> FloatBits {
        let decoded = decode(FloatBits::Binary32(value));
        match decoded.magnitude {
            Magnitude::Zero => FloatFormat::Binary64.zero(decoded.negative),
            Magnitude::Infinity => FloatFormat::Binary64.infinity(decoded.negative),
            Magnitude::Nan => FloatFormat::Binary64.quiet_nan(),
            Magnitude::Finite {
                significand,
                exponent,
            } => round(
                FloatFormat::Binary64,
                decoded.negative,
                &BigNat::from_u128(u128::from(significand)),
                &BigNat::one(),
                exponent,
            ),
        }
    }
}

fn decode(value: FloatBits) -> Decoded {
    let format = value.format();
    let raw = value.raw();
    let negative = raw & format.sign_mask() != 0;
    let exponent_field = (raw & format.exponent_mask()) >> format.fraction_bits();
    let fraction = raw & format.fraction_mask();
    let maximum_exponent = (1_u64 << format.exponent_bits()) - 1;
    let magnitude = if exponent_field == maximum_exponent {
        if fraction == 0 {
            Magnitude::Infinity
        } else {
            Magnitude::Nan
        }
    } else if exponent_field == 0 {
        if fraction == 0 {
            Magnitude::Zero
        } else {
            Magnitude::Finite {
                significand: fraction,
                exponent: format.minimum_normal_exponent()
                    - i32::try_from(format.fraction_bits()).expect("IEEE fraction width"),
            }
        }
    } else {
        Magnitude::Finite {
            significand: (1_u64 << format.fraction_bits()) | fraction,
            exponent: i32::try_from(exponent_field).expect("IEEE exponent field")
                - format.exponent_bias()
                - i32::try_from(format.fraction_bits()).expect("IEEE fraction width"),
        }
    };
    Decoded {
        negative,
        magnitude,
    }
}

fn add(format: FloatFormat, left: Decoded, right: Decoded) -> FloatBits {
    match (left.magnitude, right.magnitude) {
        (Magnitude::Nan, _) | (_, Magnitude::Nan) => format.quiet_nan(),
        (Magnitude::Infinity, Magnitude::Infinity) if left.negative != right.negative => {
            format.quiet_nan()
        }
        (Magnitude::Infinity, _) => format.infinity(left.negative),
        (_, Magnitude::Infinity) => format.infinity(right.negative),
        (Magnitude::Zero, Magnitude::Zero) => format.zero(left.negative && right.negative),
        (Magnitude::Zero, _) => encode_decoded(format, right),
        (_, Magnitude::Zero) => encode_decoded(format, left),
        (
            Magnitude::Finite {
                significand: left_significand,
                exponent: left_exponent,
            },
            Magnitude::Finite {
                significand: right_significand,
                exponent: right_exponent,
            },
        ) => {
            let exponent = left_exponent.min(right_exponent);
            let mut left_magnitude = BigNat::from_u128(u128::from(left_significand))
                .shifted(nonnegative_shift(left_exponent - exponent));
            let right_magnitude = BigNat::from_u128(u128::from(right_significand))
                .shifted(nonnegative_shift(right_exponent - exponent));
            let negative = if left.negative == right.negative {
                left_magnitude.add_assign(&right_magnitude);
                left.negative
            } else {
                match left_magnitude.cmp(&right_magnitude) {
                    Ordering::Greater => {
                        left_magnitude.subtract_assign(&right_magnitude);
                        left.negative
                    }
                    Ordering::Less => {
                        let mut result = right_magnitude;
                        result.subtract_assign(&left_magnitude);
                        left_magnitude = result;
                        right.negative
                    }
                    Ordering::Equal => return format.zero(false),
                }
            };
            round(format, negative, &left_magnitude, &BigNat::one(), exponent)
        }
    }
}

fn multiply(format: FloatFormat, left: Decoded, right: Decoded) -> FloatBits {
    let negative = left.negative ^ right.negative;
    match (left.magnitude, right.magnitude) {
        (Magnitude::Nan, _) | (_, Magnitude::Nan) => format.quiet_nan(),
        (Magnitude::Infinity, Magnitude::Zero) | (Magnitude::Zero, Magnitude::Infinity) => {
            format.quiet_nan()
        }
        (Magnitude::Infinity, _) | (_, Magnitude::Infinity) => format.infinity(negative),
        (Magnitude::Zero, _) | (_, Magnitude::Zero) => format.zero(negative),
        (
            Magnitude::Finite {
                significand: left,
                exponent: left_exponent,
            },
            Magnitude::Finite {
                significand: right,
                exponent: right_exponent,
            },
        ) => round(
            format,
            negative,
            &BigNat::from_u128(u128::from(left) * u128::from(right)),
            &BigNat::one(),
            left_exponent + right_exponent,
        ),
    }
}

fn divide(format: FloatFormat, left: Decoded, right: Decoded) -> FloatBits {
    let negative = left.negative ^ right.negative;
    match (left.magnitude, right.magnitude) {
        (Magnitude::Nan, _) | (_, Magnitude::Nan) => format.quiet_nan(),
        (Magnitude::Infinity, Magnitude::Infinity) | (Magnitude::Zero, Magnitude::Zero) => {
            format.quiet_nan()
        }
        (Magnitude::Infinity, _) | (_, Magnitude::Zero) => format.infinity(negative),
        (Magnitude::Zero, _) | (_, Magnitude::Infinity) => format.zero(negative),
        (
            Magnitude::Finite {
                significand: left,
                exponent: left_exponent,
            },
            Magnitude::Finite {
                significand: right,
                exponent: right_exponent,
            },
        ) => round(
            format,
            negative,
            &BigNat::from_u128(u128::from(left)),
            &BigNat::from_u128(u128::from(right)),
            left_exponent - right_exponent,
        ),
    }
}

fn remainder(format: FloatFormat, left: Decoded, right: Decoded) -> FloatBits {
    match (left.magnitude, right.magnitude) {
        (Magnitude::Nan | Magnitude::Infinity, _) | (_, Magnitude::Nan | Magnitude::Zero) => {
            format.quiet_nan()
        }
        (Magnitude::Zero, _) => format.zero(left.negative),
        (_, Magnitude::Infinity) => encode_decoded(format, left),
        (
            Magnitude::Finite {
                significand: left_significand,
                exponent: left_exponent,
            },
            Magnitude::Finite {
                significand: right_significand,
                exponent: right_exponent,
            },
        ) => {
            let common_exponent = left_exponent.min(right_exponent);
            let numerator = BigNat::from_u128(u128::from(left_significand))
                .shifted(nonnegative_shift(left_exponent - common_exponent));
            let denominator = BigNat::from_u128(u128::from(right_significand))
                .shifted(nonnegative_shift(right_exponent - common_exponent));
            let remainder = numerator
                .remainder(&denominator)
                .expect("finite remainder denominator is nonzero");
            if remainder.is_zero() {
                format.zero(left.negative)
            } else {
                round(
                    format,
                    left.negative,
                    &remainder,
                    &BigNat::one(),
                    common_exponent,
                )
            }
        }
    }
}

fn equal(left: Decoded, right: Decoded) -> bool {
    match (left.magnitude, right.magnitude) {
        (Magnitude::Zero, Magnitude::Zero) => true,
        (Magnitude::Infinity, Magnitude::Infinity) => left.negative == right.negative,
        (
            Magnitude::Finite {
                significand: left_significand,
                exponent: left_exponent,
            },
            Magnitude::Finite {
                significand: right_significand,
                exponent: right_exponent,
            },
        ) => {
            left.negative == right.negative
                && compare_finite_magnitude(
                    left_significand,
                    left_exponent,
                    right_significand,
                    right_exponent,
                ) == Ordering::Equal
        }
        _ => false,
    }
}

fn less(left: Decoded, right: Decoded) -> bool {
    if matches!(left.magnitude, Magnitude::Zero) && matches!(right.magnitude, Magnitude::Zero) {
        return false;
    }
    if left.negative != right.negative {
        return left.negative;
    }
    let ordering = compare_magnitude(left.magnitude, right.magnitude);
    if left.negative {
        ordering == Ordering::Greater
    } else {
        ordering == Ordering::Less
    }
}

fn compare_magnitude(left: Magnitude, right: Magnitude) -> Ordering {
    match (left, right) {
        (Magnitude::Zero, Magnitude::Zero) | (Magnitude::Infinity, Magnitude::Infinity) => {
            Ordering::Equal
        }
        (Magnitude::Zero, _) | (_, Magnitude::Infinity) => Ordering::Less,
        (_, Magnitude::Zero) | (Magnitude::Infinity, _) => Ordering::Greater,
        (
            Magnitude::Finite {
                significand: left,
                exponent: left_exponent,
            },
            Magnitude::Finite {
                significand: right,
                exponent: right_exponent,
            },
        ) => compare_finite_magnitude(left, left_exponent, right, right_exponent),
        (Magnitude::Nan, _) | (_, Magnitude::Nan) => unreachable!("NaN comparison is filtered"),
    }
}

fn compare_finite_magnitude(
    left: u64,
    left_exponent: i32,
    right: u64,
    right_exponent: i32,
) -> Ordering {
    let common = left_exponent.min(right_exponent);
    BigNat::from_u128(u128::from(left))
        .shifted(nonnegative_shift(left_exponent - common))
        .cmp(
            &BigNat::from_u128(u128::from(right))
                .shifted(nonnegative_shift(right_exponent - common)),
        )
}

fn nonnegative_shift(value: i32) -> usize {
    usize::try_from(value).expect("difference from the minimum exponent is nonnegative")
}

fn encode_decoded(format: FloatFormat, value: Decoded) -> FloatBits {
    match value.magnitude {
        Magnitude::Zero => format.zero(value.negative),
        Magnitude::Infinity => format.infinity(value.negative),
        Magnitude::Nan => format.quiet_nan(),
        Magnitude::Finite {
            significand,
            exponent,
        } => round(
            format,
            value.negative,
            &BigNat::from_u128(u128::from(significand)),
            &BigNat::one(),
            exponent,
        ),
    }
}

fn round(
    format: FloatFormat,
    negative: bool,
    numerator: &BigNat,
    denominator: &BigNat,
    binary_scale: i32,
) -> FloatBits {
    if numerator.is_zero() {
        return format.zero(negative);
    }
    let mut exponent = super::binary_exponent(numerator, denominator) + binary_scale;
    if exponent > format.maximum_normal_exponent() {
        return format.infinity(negative);
    }
    let precision = format.precision();
    let minimum = format.minimum_normal_exponent();
    let significand = if exponent >= minimum {
        let shift = precision - 1 - (exponent - binary_scale);
        let mut significand = rounded_ratio(numerator, denominator, shift)
            .expect("IEEE extraction quotient fits in u64");
        let limit = 1_u64 << u32::try_from(precision).expect("IEEE precision");
        if significand == limit {
            significand >>= 1;
            exponent += 1;
            if exponent > format.maximum_normal_exponent() {
                return format.infinity(negative);
            }
        }
        significand
    } else {
        let shift = binary_scale - minimum + precision - 1;
        let significand = rounded_ratio(numerator, denominator, shift)
            .expect("IEEE subnormal extraction quotient fits in u64");
        if significand == 0 {
            return format.zero(negative);
        }
        significand
    };
    let implicit = 1_u64 << format.fraction_bits();
    let (exponent_field, fraction) = if exponent < minimum && significand < implicit {
        (0, significand)
    } else {
        let exponent_field = u64::try_from(exponent.max(minimum) + format.exponent_bias())
            .expect("finite IEEE exponent field");
        (exponent_field, significand - implicit)
    };
    let raw = (if negative { format.sign_mask() } else { 0 })
        | (exponent_field << format.fraction_bits())
        | fraction;
    FloatBits::from_raw(format, raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nocter_model::CompilationTarget;

    fn evaluator() -> TargetFloatEvaluator {
        TargetFloatEvaluator::new(CompilationTarget::Arm64Darwin)
    }

    #[test]
    fn arithmetic_uses_target_bits_only() {
        let one = FloatBits::Binary64(0x3ff0_0000_0000_0000);
        let two = FloatBits::Binary64(0x4000_0000_0000_0000);
        assert_eq!(
            evaluator().binary(FloatBinaryOperation::Add, one, one),
            Some(two)
        );
        assert_eq!(
            evaluator().binary(FloatBinaryOperation::Divide, one, two),
            Some(FloatBits::Binary64(0x3fe0_0000_0000_0000))
        );
        assert_eq!(
            evaluator().binary(FloatBinaryOperation::Remainder, two, one),
            Some(FloatBits::Binary64(0))
        );
    }

    #[test]
    fn exceptional_values_follow_ieee_comparison_and_arithmetic() {
        let zero = FloatBits::Binary32(0);
        let negative_zero = FloatBits::Binary32(0x8000_0000);
        let infinity = FloatBits::Binary32(0x7f80_0000);
        let nan = evaluator()
            .binary(FloatBinaryOperation::Divide, zero, zero)
            .unwrap();
        assert_eq!(nan, FloatBits::Binary32(0x7fc0_0000));
        assert_eq!(
            evaluator().compare(FloatComparisonOperation::Equal, zero, negative_zero),
            Some(true)
        );
        assert_eq!(
            evaluator().compare(FloatComparisonOperation::Less, nan, infinity),
            Some(false)
        );
    }

    #[test]
    fn rounding_handles_overflow_underflow_and_subnormal_boundaries() {
        let maximum = FloatBits::Binary32(0x7f7f_ffff);
        let two = FloatBits::Binary32(0x4000_0000);
        assert_eq!(
            evaluator().binary(FloatBinaryOperation::Multiply, maximum, two),
            Some(FloatBits::Binary32(0x7f80_0000))
        );
        assert_eq!(
            evaluator().binary(FloatBinaryOperation::Divide, FloatBits::Binary32(1), two),
            Some(FloatBits::Binary32(0))
        );
    }

    #[test]
    fn binary32_matches_ieee_reference_vectors() {
        let values = [
            0x0000_0000,
            0x8000_0000,
            0x0000_0001,
            0x007f_ffff,
            0x0080_0000,
            0x3dcc_cccd,
            0x3f00_0000,
            0x3f80_0000,
            0xbf80_0000,
            0x7f7f_ffff,
            0xff7f_ffff,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_1234,
        ];
        for left in values {
            for right in values {
                for operation in [
                    FloatBinaryOperation::Add,
                    FloatBinaryOperation::Subtract,
                    FloatBinaryOperation::Multiply,
                    FloatBinaryOperation::Divide,
                    FloatBinaryOperation::Remainder,
                ] {
                    let FloatBits::Binary32(actual) = evaluator()
                        .binary(
                            operation,
                            FloatBits::Binary32(left),
                            FloatBits::Binary32(right),
                        )
                        .unwrap()
                    else {
                        unreachable!("binary32 inputs produce a binary32 result")
                    };
                    let left = f32::from_bits(left);
                    let right = f32::from_bits(right);
                    let expected = match operation {
                        FloatBinaryOperation::Add => left + right,
                        FloatBinaryOperation::Subtract => left - right,
                        FloatBinaryOperation::Multiply => left * right,
                        FloatBinaryOperation::Divide => left / right,
                        FloatBinaryOperation::Remainder => left % right,
                    };
                    assert_same_binary32(actual, expected.to_bits(), operation, left, right);
                }
            }
        }
    }

    #[test]
    fn binary64_matches_ieee_reference_vectors() {
        let values = [
            0x0000_0000_0000_0000,
            0x8000_0000_0000_0000,
            0x0000_0000_0000_0001,
            0x000f_ffff_ffff_ffff,
            0x0010_0000_0000_0000,
            0x3fb9_9999_9999_999a,
            0x3fe0_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0xbff0_0000_0000_0000,
            0x7fef_ffff_ffff_ffff,
            0xffef_ffff_ffff_ffff,
            0x7ff0_0000_0000_0000,
            0xfff0_0000_0000_0000,
            0x7ff8_0000_0000_1234,
        ];
        for left in values {
            for right in values {
                for operation in [
                    FloatBinaryOperation::Add,
                    FloatBinaryOperation::Subtract,
                    FloatBinaryOperation::Multiply,
                    FloatBinaryOperation::Divide,
                    FloatBinaryOperation::Remainder,
                ] {
                    let actual = evaluator()
                        .binary(
                            operation,
                            FloatBits::Binary64(left),
                            FloatBits::Binary64(right),
                        )
                        .unwrap()
                        .raw();
                    let left = f64::from_bits(left);
                    let right = f64::from_bits(right);
                    let expected = match operation {
                        FloatBinaryOperation::Add => left + right,
                        FloatBinaryOperation::Subtract => left - right,
                        FloatBinaryOperation::Multiply => left * right,
                        FloatBinaryOperation::Divide => left / right,
                        FloatBinaryOperation::Remainder => left % right,
                    };
                    assert_same_binary64(actual, expected.to_bits(), operation, left, right);
                }
            }
        }
    }

    fn assert_same_binary32(
        actual: u32,
        expected: u32,
        operation: FloatBinaryOperation,
        left: f32,
        right: f32,
    ) {
        if f32::from_bits(expected).is_nan() {
            assert!(
                f32::from_bits(actual).is_nan(),
                "{operation:?}({left:?}, {right:?}) produced {actual:#010x}"
            );
        } else {
            assert_eq!(actual, expected, "{operation:?}({left:?}, {right:?})");
        }
    }

    fn assert_same_binary64(
        actual: u64,
        expected: u64,
        operation: FloatBinaryOperation,
        left: f64,
        right: f64,
    ) {
        if f64::from_bits(expected).is_nan() {
            assert!(
                f64::from_bits(actual).is_nan(),
                "{operation:?}({left:?}, {right:?}) produced {actual:#018x}"
            );
        } else {
            assert_eq!(actual, expected, "{operation:?}({left:?}, {right:?})");
        }
    }
}
