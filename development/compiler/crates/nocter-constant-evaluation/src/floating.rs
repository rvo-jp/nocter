use std::cmp::Ordering;

use nocter_model::CompilationTarget;
use nocter_syntax::FloatLiteralSuffix;

mod operations;

pub use operations::{FloatBinaryOperation, FloatComparisonOperation};

/// IEEE binary format selected after contextual source typing.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatFormat {
    Binary32,
    Binary64,
}

impl FloatFormat {
    const fn precision(self) -> i32 {
        match self {
            Self::Binary32 => 24,
            Self::Binary64 => 53,
        }
    }

    const fn minimum_normal_exponent(self) -> i32 {
        match self {
            Self::Binary32 => -126,
            Self::Binary64 => -1022,
        }
    }

    const fn maximum_normal_exponent(self) -> i32 {
        match self {
            Self::Binary32 => 127,
            Self::Binary64 => 1023,
        }
    }

    const fn exponent_bias(self) -> i32 {
        match self {
            Self::Binary32 => 127,
            Self::Binary64 => 1023,
        }
    }

    const fn fraction_bits(self) -> u32 {
        match self {
            Self::Binary32 => 23,
            Self::Binary64 => 52,
        }
    }
}

impl From<FloatLiteralSuffix> for FloatFormat {
    fn from(suffix: FloatLiteralSuffix) -> Self {
        match suffix {
            FloatLiteralSuffix::F32 => Self::Binary32,
            FloatLiteralSuffix::F64 => Self::Binary64,
        }
    }
}

/// A typed IEEE value represented only by target bits.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatBits {
    Binary32(u32),
    Binary64(u64),
}

/// Failure to convert one lexically valid, unsigned decimal literal.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FloatLiteralError {
    Invalid,
    Overflow,
    Underflow,
}

/// The sole target-parameterized decimal-to-IEEE conversion authority.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TargetFloatEvaluator {
    target: CompilationTarget,
}

impl TargetFloatEvaluator {
    #[must_use]
    pub const fn new(target: CompilationTarget) -> Self {
        Self { target }
    }

    #[must_use]
    pub const fn target(self) -> CompilationTarget {
        self.target
    }

    /// Converts one unsigned decimal spelling using round-to-nearest, ties-to-even.
    ///
    /// The implementation performs integer-rational rounding and never asks the compiler host to
    /// parse or calculate a floating-point value. The target parameter deliberately remains part
    /// of this contract even while every recognized target uses the same IEEE scalar formats.
    ///
    /// # Errors
    ///
    /// Returns [`FloatLiteralError`] when the spelling is invalid or its nonzero value is outside
    /// the finite range of the selected format.
    pub fn decimal_bits(
        self,
        spelling: &str,
        format: FloatFormat,
    ) -> Result<FloatBits, FloatLiteralError> {
        let decimal = Decimal::parse(spelling)?;
        let bits = decimal.to_ieee(format)?;
        match format {
            FloatFormat::Binary32 => u32::try_from(bits)
                .map(FloatBits::Binary32)
                .map_err(|_| FloatLiteralError::Invalid),
            FloatFormat::Binary64 => Ok(FloatBits::Binary64(bits)),
        }
    }
}

struct Decimal {
    significand: BigNat,
    significant_decimal_digits: i64,
    exponent10: i64,
}

impl Decimal {
    fn parse(spelling: &str) -> Result<Self, FloatLiteralError> {
        let (mantissa, authored_exponent, has_exponent) = split_exponent(spelling)?;
        if !mantissa.contains('.') && !has_exponent {
            return Err(FloatLiteralError::Invalid);
        }
        let (whole, fraction) = match mantissa.split_once('.') {
            Some((whole, fraction)) if !fraction.contains('.') => (whole, Some(fraction)),
            Some(_) => return Err(FloatLiteralError::Invalid),
            None => (mantissa, None),
        };
        if !valid_decimal_group(whole)
            || fraction.is_some_and(|fraction| !valid_decimal_group(fraction))
        {
            return Err(FloatLiteralError::Invalid);
        }
        let fraction_digits = fraction
            .unwrap_or_default()
            .bytes()
            .filter(u8::is_ascii_digit)
            .count();
        let mut digits = whole
            .bytes()
            .chain(fraction.unwrap_or_default().bytes())
            .filter(u8::is_ascii_digit)
            .map(|byte| byte - b'0')
            .collect::<Vec<_>>();
        let first_nonzero = digits.iter().position(|digit| *digit != 0);
        let Some(first_nonzero) = first_nonzero else {
            return Ok(Self {
                significand: BigNat::zero(),
                significant_decimal_digits: 1,
                exponent10: 0,
            });
        };
        digits.drain(..first_nonzero);
        let trailing_zeros = digits.iter().rev().take_while(|digit| **digit == 0).count();
        digits.truncate(digits.len() - trailing_zeros);

        let exponent10 = authored_exponent
            .saturating_sub(i64::try_from(fraction_digits).unwrap_or(i64::MAX))
            .saturating_add(i64::try_from(trailing_zeros).unwrap_or(i64::MAX));
        let significant_decimal_digits =
            i64::try_from(digits.len()).map_err(|_| FloatLiteralError::Invalid)?;
        Ok(Self {
            significand: BigNat::from_decimal_digits(&digits),
            significant_decimal_digits,
            exponent10,
        })
    }

    fn to_ieee(&self, format: FloatFormat) -> Result<u64, FloatLiteralError> {
        if self.significand.is_zero() {
            return Ok(0);
        }
        let adjusted = self
            .significant_decimal_digits
            .saturating_add(self.exponent10)
            .saturating_sub(1);
        if adjusted > 400 {
            return Err(FloatLiteralError::Overflow);
        }
        if adjusted < -500 {
            return Err(FloatLiteralError::Underflow);
        }

        let (numerator, denominator) = if self.exponent10 >= 0 {
            let exponent =
                usize::try_from(self.exponent10).map_err(|_| FloatLiteralError::Overflow)?;
            (
                self.significand
                    .clone()
                    .multiplied_by_power_of_ten(exponent),
                BigNat::one(),
            )
        } else {
            let exponent = usize::try_from(self.exponent10.unsigned_abs())
                .map_err(|_| FloatLiteralError::Underflow)?;
            (self.significand.clone(), BigNat::power_of_ten(exponent))
        };
        let mut exponent = binary_exponent(&numerator, &denominator);
        let precision = format.precision();
        let minimum = format.minimum_normal_exponent();
        let maximum = format.maximum_normal_exponent();

        if exponent >= minimum {
            let shift = precision - 1 - exponent;
            let mut significand = rounded_ratio(&numerator, &denominator, shift)?;
            let precision_limit = 1_u64 << u32::try_from(precision).expect("IEEE precision");
            if significand == precision_limit {
                significand >>= 1;
                exponent += 1;
            }
            if exponent > maximum {
                return Err(FloatLiteralError::Overflow);
            }
            let implicit = 1_u64 << format.fraction_bits();
            let exponent_field = u64::try_from(exponent + format.exponent_bias())
                .expect("normal IEEE exponent is nonnegative");
            return Ok((exponent_field << format.fraction_bits()) | (significand - implicit));
        }

        let shift = precision - 1 - minimum;
        let significand = rounded_ratio(&numerator, &denominator, shift)?;
        if significand == 0 {
            return Err(FloatLiteralError::Underflow);
        }
        let implicit = 1_u64 << format.fraction_bits();
        if significand >= implicit {
            Ok(1_u64 << format.fraction_bits())
        } else {
            Ok(significand)
        }
    }
}

fn split_exponent(spelling: &str) -> Result<(&str, i64, bool), FloatLiteralError> {
    let Some(index) = spelling.find(['e', 'E']) else {
        return Ok((spelling, 0, false));
    };
    if spelling[index + 1..].contains(['e', 'E']) {
        return Err(FloatLiteralError::Invalid);
    }
    let authored = &spelling[index + 1..];
    let digits = authored.strip_prefix(['+', '-']).unwrap_or(authored);
    if !valid_decimal_group(digits) {
        return Err(FloatLiteralError::Invalid);
    }
    let negative = authored.starts_with('-');
    let magnitude = digits
        .bytes()
        .filter(u8::is_ascii_digit)
        .fold(0_i64, |value, digit| {
            value
                .saturating_mul(10)
                .saturating_add(i64::from(digit - b'0'))
        });
    let value = if negative {
        magnitude.saturating_neg()
    } else {
        magnitude
    };
    Ok((&spelling[..index], value, true))
}

fn valid_decimal_group(text: &str) -> bool {
    let bytes = text.as_bytes();
    !bytes.is_empty()
        && bytes.iter().enumerate().all(|(index, byte)| {
            byte.is_ascii_digit()
                || (*byte == b'_'
                    && index > 0
                    && index + 1 < bytes.len()
                    && bytes[index - 1].is_ascii_digit()
                    && bytes[index + 1].is_ascii_digit())
        })
}

fn binary_exponent(numerator: &BigNat, denominator: &BigNat) -> i32 {
    let candidate = i32::try_from(numerator.bit_len()).expect("bounded decimal numerator")
        - i32::try_from(denominator.bit_len()).expect("bounded decimal denominator");
    let comparison = if candidate >= 0 {
        numerator.cmp(&denominator.shifted(candidate.unsigned_abs() as usize))
    } else {
        numerator
            .shifted(candidate.unsigned_abs() as usize)
            .cmp(denominator)
    };
    if comparison == Ordering::Less {
        candidate - 1
    } else {
        candidate
    }
}

fn rounded_ratio(
    numerator: &BigNat,
    denominator: &BigNat,
    binary_shift: i32,
) -> Result<u64, FloatLiteralError> {
    let (mut scaled_numerator, scaled_denominator) = if binary_shift >= 0 {
        (
            numerator.shifted(binary_shift.unsigned_abs() as usize),
            denominator.clone(),
        )
    } else {
        (
            numerator.clone(),
            denominator.shifted(binary_shift.unsigned_abs() as usize),
        )
    };
    let quotient = scaled_numerator.divide_to_u64(&scaled_denominator)?;
    let twice_remainder = scaled_numerator.shifted(1);
    let round_up = match twice_remainder.cmp(&scaled_denominator) {
        Ordering::Greater => true,
        Ordering::Equal => quotient % 2 == 1,
        Ordering::Less => false,
    };
    quotient
        .checked_add(u64::from(round_up))
        .ok_or(FloatLiteralError::Overflow)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BigNat {
    limbs: Vec<u32>,
}

impl BigNat {
    const fn zero() -> Self {
        Self { limbs: Vec::new() }
    }

    fn one() -> Self {
        Self { limbs: vec![1] }
    }

    fn from_decimal_digits(digits: &[u8]) -> Self {
        let mut value = Self::zero();
        for digit in digits {
            value.multiply_small(10);
            value.add_small(u32::from(*digit));
        }
        value
    }

    fn from_u128(value: u128) -> Self {
        let limbs = [
            limb_from_u128(value),
            limb_from_u128(value >> 32),
            limb_from_u128(value >> 64),
            limb_from_u128(value >> 96),
        ];
        let length = limbs
            .iter()
            .rposition(|limb| *limb != 0)
            .map_or(0, |index| index + 1);
        Self {
            limbs: limbs[..length].to_vec(),
        }
    }

    fn power_of_ten(exponent: usize) -> Self {
        Self::one().multiplied_by_power_of_ten(exponent)
    }

    fn multiplied_by_power_of_ten(mut self, exponent: usize) -> Self {
        for _ in 0..exponent {
            self.multiply_small(10);
        }
        self
    }

    fn multiply_small(&mut self, multiplier: u32) {
        let mut carry = 0_u64;
        for limb in &mut self.limbs {
            let value = u64::from(*limb) * u64::from(multiplier) + carry;
            *limb = low_limb(value);
            carry = value >> 32;
        }
        if carry != 0 {
            self.limbs.push(low_limb(carry));
        }
    }

    fn add_small(&mut self, addend: u32) {
        let mut carry = u64::from(addend);
        for limb in &mut self.limbs {
            let value = u64::from(*limb) + carry;
            *limb = low_limb(value);
            carry = value >> 32;
            if carry == 0 {
                return;
            }
        }
        if carry != 0 {
            self.limbs.push(low_limb(carry));
        }
    }

    fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    fn bit_len(&self) -> usize {
        self.limbs.last().map_or(0, |last| {
            (self.limbs.len() - 1) * 32 + (32 - last.leading_zeros() as usize)
        })
    }

    fn shifted(&self, bits: usize) -> Self {
        if self.is_zero() {
            return Self::zero();
        }
        let whole = bits / 32;
        let partial = bits % 32;
        let mut limbs = vec![0; whole];
        let mut carry = 0_u64;
        for limb in &self.limbs {
            let value = (u64::from(*limb) << partial) | carry;
            limbs.push(low_limb(value));
            carry = value >> 32;
        }
        if carry != 0 {
            limbs.push(low_limb(carry));
        }
        Self { limbs }
    }

    fn subtract_assign(&mut self, other: &Self) {
        debug_assert!((*self).cmp(other) != Ordering::Less);
        let mut borrow = 0_i64;
        for index in 0..self.limbs.len() {
            let left = i64::from(self.limbs[index]);
            let right = other.limbs.get(index).copied().map_or(0, i64::from);
            let value = left - right - borrow;
            self.limbs[index] = u32::try_from(value.rem_euclid(1_i64 << 32))
                .expect("one limb subtraction stays within the limb modulus");
            borrow = i64::from(value < 0);
        }
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }

    fn add_assign(&mut self, other: &Self) {
        let length = self.limbs.len().max(other.limbs.len());
        self.limbs.resize(length, 0);
        let mut carry = 0_u64;
        for index in 0..length {
            let value = u64::from(self.limbs[index])
                + u64::from(other.limbs.get(index).copied().unwrap_or(0))
                + carry;
            self.limbs[index] = low_limb(value);
            carry = value >> 32;
        }
        if carry != 0 {
            self.limbs.push(low_limb(carry));
        }
    }

    fn remainder(mut self, denominator: &Self) -> Result<Self, FloatLiteralError> {
        if denominator.is_zero() {
            return Err(FloatLiteralError::Invalid);
        }
        while self.cmp(denominator) != Ordering::Less {
            let mut shift = self.bit_len() - denominator.bit_len();
            let mut shifted = denominator.shifted(shift);
            if self.cmp(&shifted) == Ordering::Less {
                shift -= 1;
                shifted = denominator.shifted(shift);
            }
            self.subtract_assign(&shifted);
        }
        Ok(self)
    }

    /// Divides in place, leaving the remainder and returning a quotient known to fit in `u64`.
    fn divide_to_u64(&mut self, denominator: &Self) -> Result<u64, FloatLiteralError> {
        if denominator.is_zero() {
            return Err(FloatLiteralError::Invalid);
        }
        if (*self).cmp(denominator) == Ordering::Less {
            return Ok(0);
        }
        let highest = self.bit_len() - denominator.bit_len();
        if highest >= 64 {
            return Err(FloatLiteralError::Overflow);
        }
        let mut quotient = 0_u64;
        for bit in (0..=highest).rev() {
            let shifted = denominator.shifted(bit);
            if (*self).cmp(&shifted) != Ordering::Less {
                self.subtract_assign(&shifted);
                quotient |= 1_u64 << bit;
            }
        }
        Ok(quotient)
    }
}

fn low_limb(value: u64) -> u32 {
    u32::try_from(value & u64::from(u32::MAX)).expect("masked value fits one limb")
}

fn limb_from_u128(value: u128) -> u32 {
    u32::try_from(value & u128::from(u32::MAX)).expect("masked value fits one limb")
}

impl Ord for BigNat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.limbs
            .len()
            .cmp(&other.limbs.len())
            .then_with(|| self.limbs.iter().rev().cmp(other.limbs.iter().rev()))
    }
}

impl PartialOrd for BigNat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f32_bits(spelling: &str) -> Result<u32, FloatLiteralError> {
        match TargetFloatEvaluator::new(CompilationTarget::Arm64Darwin)
            .decimal_bits(spelling, FloatFormat::Binary32)?
        {
            FloatBits::Binary32(bits) => Ok(bits),
            FloatBits::Binary64(_) => unreachable!(),
        }
    }

    fn f64_bits(spelling: &str) -> Result<u64, FloatLiteralError> {
        match TargetFloatEvaluator::new(CompilationTarget::Arm64Darwin)
            .decimal_bits(spelling, FloatFormat::Binary64)?
        {
            FloatBits::Binary64(bits) => Ok(bits),
            FloatBits::Binary32(_) => unreachable!(),
        }
    }

    #[test]
    fn converts_decimal_values_without_host_floating_point() {
        assert_eq!(f32_bits("0.0"), Ok(0));
        assert_eq!(f32_bits("0.1"), Ok(0x3dcc_cccd));
        assert_eq!(f64_bits("1.0"), Ok(0x3ff0_0000_0000_0000));
        assert_eq!(f64_bits("0.1"), Ok(0x3fb9_9999_9999_999a));
        assert_eq!(f64_bits("0e999999999999999999999999"), Ok(0));
        assert_eq!(
            f64_bits("1.7976931348623157e308"),
            Ok(0x7fef_ffff_ffff_ffff)
        );
        assert_eq!(f64_bits("5e-324"), Ok(1));
        assert_eq!(f32_bits("1.40129846e-45"), Ok(1));
    }

    #[test]
    fn rounds_exact_halfway_values_to_even() {
        assert_eq!(
            f64_bits("1.00000000000000011102230246251565404236316680908203125"),
            Ok(0x3ff0_0000_0000_0000)
        );
        assert_eq!(
            f64_bits("1.00000000000000011102230246251565404236316680908203126"),
            Ok(0x3ff0_0000_0000_0001)
        );
    }

    #[test]
    fn rejects_nonzero_values_that_overflow_or_round_to_zero() {
        assert_eq!(
            f64_bits("1.7976931348623159e308"),
            Err(FloatLiteralError::Overflow)
        );
        assert_eq!(f64_bits("2e-324"), Err(FloatLiteralError::Underflow));
        assert_eq!(f32_bits("1e39"), Err(FloatLiteralError::Overflow));
        assert_eq!(f32_bits("1e-50"), Err(FloatLiteralError::Underflow));
        assert_eq!(f64_bits("1"), Err(FloatLiteralError::Invalid));
    }
}
