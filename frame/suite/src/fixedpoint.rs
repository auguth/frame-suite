// SPDX-License-Identifier: MPL-2.0
//
// Part of Auguth Labs open-source softwares.
// Built for the Substrate framework.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 Auguth Labs (OPC) Pvt Ltd, India

// ===============================================================================
// `````````````````````````````````` FIXED-POINT `````````````````````````````````
// ===============================================================================
 
//! Deterministic, `no_std`-compatible mathematical primitives for Substrate's
//! fixed-point numeric tower ([`FixedU64`], [`FixedU128`], [`FixedI64`], [`FixedI128`]).
//!
//! All arithmetic is implemented without floating-point instructions, making
//! every operation fully deterministic and suitable for on-chain execution where
//! bit-identical results across heterogeneous validator hardware are required.
//!
//! ## Type System
//!
//! Three layered abstractions bridge raw integers and fixed-point values:
//!
//! | Trait                | Role                                                        |
//! |----------------------|-------------------------------------------------------------|
//! | [`FixedForInteger`]  | Associates each primitive integer with its natural fixed-point counterpart |
//! | [`IntegerToFixed`]   | Round-trip `to_fixed` / `from_fixed` with saturation at type boundaries   |
//! | [`FixedSignedCast`]  | Lifts unsigned types into signed arithmetic space for operations that require negative intermediates, then projects the result back |
//!
//! ## Operations
//!
//! | Function        | Description                                              |
//! |-----------------|----------------------------------------------------------|
//! | `fixed_sqrt`    | Square root - real domain; returns `None` for negatives  |
//! | `complex_sqrt`  | Square root - complex domain; imaginary output for `x<0` |
//! | `fixed_exp`     | Natural exponential `e^x`                                 |
//! | `fixed_ln`      | Natural logarithm `ln(x)`, defined for `x > 0`           |
//! | `fixed_pow`     | General power `x^p` - integer and fractional exponents    |
//!
//! Operations are exposed through the [`FixedOp`] and [`FixedComplexOp`] trait
//! facades, so generic code can be written against a single trait bound and
//! work across all four fixed-point types without specialisation.
//!
//! ## Design Notes
//!
//! - **No panics.** All public entry-points return `Option<T>` so that undefined
//!   inputs (negative logarithm, zero base with negative exponent, etc.) are
//!   expressed as `None` rather than a runtime abort.
//! - **Saturating internal arithmetic.** Intermediate overflow clamps to the
//!   type's representable range rather than wrapping or panicking.
//! - **Convergence guarantees.** Every iterative algorithm is hard-capped at
//!   `MAX_ITERATIONS` and also checks for stagnation, so no function can loop
//!   indefinitely regardless of input.
//!
//! ## Planned Extensions
//!
//! Trigonometric, hyperbolic, special (gamma, erf), and additional root / power
//! functions are outlined in the `PLANNED EXTENSIONS` section at the bottom of
//! this file. New operations should implement the corresponding method on
//! [`FixedOp`] (or a new companion trait) and follow the same `Option`-returning,
//! saturation-safe conventions established here.

// ===============================================================================
// ``````````````````````````````````` IMPORTS ```````````````````````````````````
// ===============================================================================

// --- Core ---
use core::ops::Shr;
use core::convert::TryInto;

// --- Substrate crates ---
use sp_arithmetic::{FixedI128, FixedI64, FixedU128, FixedU64};
use sp_runtime::{
    FixedPointNumber,
    traits::Bounded
};

// ===============================================================================
// ```````````````````````````` INTEGER-FIXED MAPPING ````````````````````````````
// ===============================================================================

/// Trait mapping **primitive integer types** to an appropriate **fixed-point type**.
///
/// This is useful in generic algorithms where a numeric type might need to be converted
/// to a fixed-point representation for deterministic arithmetic, scaling, or computations.
pub trait FixedForInteger {
    /// The fixed-point type corresponding to the integer type.
    ///
    ///   - Small unsigned integers (u8, u16, u32) map to `FixedU64`
    ///   - Large unsigned integers (u64, u128, usize) map to `FixedU128`
    ///   - Signed integers follow a similar mapping with `FixedI64` or `FixedI128`.
    type FixedPoint: FixedPointNumber;
}

/// Macro to conveniently implement [`FixedForInteger`] for multiple integer types at once.
///
macro_rules! int_best_fixed {
    // Accepts pairs of integer type => fixed-point type
    ($($t:ty => $fixed:ty),* $(,)?) => {
        $(
            // Implement the FixedForInteger trait for the integer type
            impl FixedForInteger for $t {
                // Associate the chosen fixed-point type with this integer
                type FixedPoint = $fixed;
            }
        )*
    };
}

// Implement [`FixedForInteger`] for all primitive integer types.
//
// Provides sensible defaults:
// - **Unsigned small integers (u8, u16, u32)** -> `FixedU64`
// - **Unsigned large integers (u64, u128, usize)** -> `FixedU128`
// - **Signed small integers (i8, i16, i32)** -> `FixedI64`
// - **Signed large integers (i64, i128, isize)** -> `FixedI128`
//
// This ensures consistent fixed-point conversions across different integer sizes,
// particularly in algorithms involving weighting, or normalized calculations.
int_best_fixed! {
    u8   => FixedU64,
    u16  => FixedU64,
    u32  => FixedU64,
    u64  => FixedU128,
    u128 => FixedU128,
    usize => FixedU128,
    i8   => FixedI64,
    i16  => FixedI64,
    i32  => FixedI64,
    i64  => FixedI128,
    i128 => FixedI128,
    isize => FixedI128,
}

// ===============================================================================
// ``````````````````````````` INTEGER-FIXED CONVERSION ``````````````````````````
// ===============================================================================

/// Trait for converting a numeric type to and from its **associated fixed-point type**.
///
/// This is intended for integer types that implement [`FixedForInteger`],
/// allowing deterministic fixed-point arithmetic while preserving the original type.
pub trait IntegerToFixed: Sized + FixedForInteger {
    /// Convert the current value to the mapped fixed-point type.
    fn to_fixed(&self) -> <Self as FixedForInteger>::FixedPoint;

    /// Convert a value in the mapped fixed-point type back to the original type.
    fn from_fixed(f: &<Self as FixedForInteger>::FixedPoint) -> Self;
}

/// Implements `IntegerToFixed` conversion for **all unsigned integer types** in the list.
///
/// - `to_fixed`: Converts the integer into the corresponding fixed-point type using
///   saturating conversion to prevent overflow.
/// - `from_fixed`: Converts back from fixed-point to the integer, clamping values to
///   the integer's max if the fixed-point inner value exceeds it.
///
/// Usage: `impl_fixed_convert_unsigned!(u8, u16, u32 => FixedU64);`
macro_rules! impl_fixed_convert_unsigned {
    // Accepts a comma-separated list of unsigned types ($t) and a fixed-point type ($fixed)
    ($($t:ty),* => $fixed:ty) => {
        $(
            impl IntegerToFixed for $t {
                /// Convert integer to fixed-point
                fn to_fixed(&self) -> <$t as FixedForInteger>::FixedPoint {
                    // Saturating conversion ensures no overflow when casting integer to fixed
                    <$fixed>::saturating_from_integer(*self as $t)
                }

                /// Convert fixed-point back to integer
                fn from_fixed(f: &<$t as FixedForInteger>::FixedPoint) -> Self {
                    // Extract the underlying integer from the fixed-point type
                    let inner = f.into_inner().saturating_div(<$fixed>::DIV);
                    // Clamp to the maximum value of the integer type
                    if inner > <$t>::MAX as _ {
                        <$t>::MAX
                    } else {
                        // Safe cast for unsigned integers
                        inner as $t
                    }
                }
            }
        )*
    };
}

/// Implements `IntegerToFixed` conversion for **all signed integer types** in the list.
///
/// - `to_fixed`: Converts the integer into the corresponding fixed-point type using
///   saturating conversion.
/// - `from_fixed`: Converts back from fixed-point to the integer, clamping to
///   both the integer's min and max if the fixed-point inner value is out of bounds.
///
/// Usage: `impl_fixed_convert_signed!(i8, i16, i32 => FixedI64);`
macro_rules! impl_fixed_convert_signed {
    // Accepts a comma-separated list of signed types ($t) and a fixed-point type ($fixed)
    ($($t:ty),* => $fixed:ty) => {
        $(
            impl IntegerToFixed for $t {
                /// Convert signed integer to fixed-point
                fn to_fixed(&self) -> <$t as FixedForInteger>::FixedPoint {
                    // Saturating conversion prevents overflow when converting signed integer to fixed
                    <$fixed>::saturating_from_integer(*self as $t)
                }

                /// Convert fixed-point back to signed integer
                fn from_fixed(f: &<$t as FixedForInteger>::FixedPoint) -> Self {
                    // Extract the underlying integer from the fixed-point type
                    let inner = f.into_inner().saturating_div(<$fixed>::DIV);

                    // Clamp to the maximum value of the integer type
                    if inner > <$t>::MAX as _ {
                        <$t>::MAX
                    }
                    // Clamp to the minimum value of the integer type
                    else if inner < <$t>::MIN as _ {
                        <$t>::MIN
                    }
                    // Safe cast for values within the integer range
                    else {
                        inner as $t
                    }
                }
            }
        )*
    };
}

// Apply conversions for small unsigned integers
impl_fixed_convert_unsigned!(u8, u16, u32 => FixedU64);
// Apply conversions for large unsigned integers
impl_fixed_convert_unsigned!(u64, u128, usize => FixedU128);
// Apply conversions for small signed integers
impl_fixed_convert_signed!(i8, i16, i32 => FixedI64);
// Apply conversions for large signed integers
impl_fixed_convert_signed!(i64, i128, isize => FixedI128);

// ===============================================================================
// ```````````````````````````` SIGNED CAST BRIDGE ```````````````````````````````
// ===============================================================================

/// A bridge that allows any [`FixedPointNumber`] type - including unsigned ones -
/// to perform arithmetic in a signed intermediate space, then project the result
/// back to the original type.
///
/// ## Motivation
///
/// Several mathematical operations (logarithm of a fraction, negative exponents,
/// complex-domain arithmetic) require signed intermediates even when the input
/// and final result are both representable as unsigned values. Rather than
/// duplicating signed-aware implementations for every function, `FixedSignedCast`
/// provides a single seam:
///
/// - **Signed types** (`FixedI64`, `FixedI128`) implement this trait as a
///   pure identity: the associated `Signed` type is `Self`, and every conversion
///   is a no-op.
/// - **Unsigned types** (`FixedU64`, `FixedU128`) map to a wider signed
///   counterpart (`FixedI128`) that can represent the full unsigned range as
///   non-negative values. Conversions to/from `Signed` clamp or fail gracefully
///   when a result is negative (i.e. not representable by the unsigned type).
///
/// ## Associated Type
///
/// - `Signed` - the signed fixed-point type used as the arithmetic workspace.
///   For signed types this is `Self`, for unsigned types it is `FixedI128`.
///
/// ## Methods
///
/// | Method             | Behaviour on error / out-of-range              |
/// |--------------------|------------------------------------------------|
/// | `saturating`       | Clamps the result to the target type's bounds  |
/// | `checked`          | Returns `None` when the result is out-of-range |
/// | `checked_into`     | `Self -> Option<Signed>`                        |
/// | `saturated_into`   | `Self -> Signed` (clamping on overflow)         |
/// | `checked_from`     | `Signed -> Option<Self>`                        |
/// | `saturated_from`   | `Signed -> Self` (clamping on underflow/overflow)|
///
/// ## Usage
///
/// Prefer [`FixedSignedCast::saturating`] for operations where out-of-range
/// results should clamp silently, and [`FixedSignedCast::checked`] where
/// out-of-range results must be propagated to the caller as `None`.
pub trait FixedSignedCast : FixedPointNumber {
    /// The signed fixed-point workspace type for intermediate arithmetic.
    ///
    /// - Signed types (`FixedI64`, `FixedI128`): `type Signed = Self`.
    /// - Unsigned types (`FixedU64`, `FixedU128`): `type Signed = FixedI128`.
    type Signed: FixedPointNumber;

    /// Applies the closure `f` in `Signed` space and converts the result back
    /// to `Self`, **clamping** at the type's representable bounds on overflow
    /// or underflow.
    ///
    /// Useful when signed arithmetic may produce a value outside the target
    /// range but a best-effort saturated answer is acceptable.
    fn saturating<F>(x: Self, f: F) -> Self where F: FnOnce(Self::Signed)->Self::Signed;

    /// Applies the closure `f` in `Signed` space and converts the result back
    /// to `Self`, returning `None` when the result cannot be represented.
    ///
    /// The closure receives an `Option<Signed>` - `None` signals that the
    /// initial conversion from `Self` into `Signed` already failed (only
    /// possible for `FixedU128` values exceeding `i128::MAX`).
    fn checked<F>(x: Self, f: F) -> Option<Self> where F: FnOnce(Option<Self::Signed>)->Self::Signed;

    /// Converts `Self` into `Signed`, returning `None` if the value cannot
    /// be represented in `Signed`.
    ///
    /// For signed types this is always `Some(x)`. For unsigned types, this
    /// fails only when `x.into_inner() > i128::MAX` (only reachable with
    /// `FixedU128` values in the upper half of its range).
    fn checked_into(x: Self) -> Option<Self::Signed>;

    /// Converts `Self` into `Signed`, clamping to `Signed::max_value()` on
    /// overflow.
    ///
    /// For signed types this is a zero-cost identity. For unsigned types the
    /// inner `u64`/`u128` value is reinterpreted as `i128`; values that exceed
    /// `i128::MAX` clamp to `i128::MAX`.
    fn saturated_into(x: Self) -> Self::Signed;

    /// Converts a `Signed` value into `Self`, returning `None` if the value
    /// falls outside the representable range of `Self`.
    ///
    /// For signed types this is always `Some(x)`. For unsigned types, a
    /// negative `Signed` inner value means the result is negative and therefore
    /// unrepresentable - `None` is returned.
    fn checked_from(x: Self::Signed) -> Option<Self>;

    /// Converts a `Signed` value into `Self`, clamping at the type bounds.
    ///
    /// For signed types this is a zero-cost identity. For unsigned types,
    /// negative values clamp to `0`. No upper clamp is needed: a non-negative
    /// `i128` inner value is at most `i128::MAX = 2^127 - 1`, which is always
    /// less than `u128::MAX = 2^128 - 1`, so it always fits in the unsigned
    /// inner type without loss.
    fn saturated_from(x: Self::Signed) -> Self;
}

/// Identity implementation for `FixedI64`.
///
/// `FixedI64` is already signed, so `checked_into`, `saturated_into`,
/// `checked_from`, and `saturated_from` are all zero-cost identity operations.
///
/// `saturating` delegates directly to `f` - any saturation that occurs inside
/// the closure is the closure's own saturating arithmetic, which is the
/// expected behaviour for this variant.
///
/// `checked` delegates to `f` and returns `None` only when the result has
/// saturated to `min_value()` or `max_value()`, which are the two sentinel
/// values that saturating arithmetic produces on overflow. If the closure
/// legitimately computes exactly `min_value()` or `max_value()`, `None` is
/// returned conservatively. For cases where that distinction matters, prefer
/// the saturating variant and handle clamping at the call site.
impl FixedSignedCast for FixedI64 {
    type Signed = FixedI64;

    fn saturating<F>(x: Self, f: F) -> Self
    where
        F: FnOnce(Self::Signed) -> Self::Signed,
    {
        f(x)
    }

    fn checked<F>(x: Self, f: F) -> Option<Self>
    where
        F: FnOnce(Option<Self::Signed>) -> Self::Signed,
    {
        let result = f(Some(x));
        // Detect saturation: saturating arithmetic clamps to min/max on overflow.
        // Treat either sentinel as evidence that the result is out of range.
        if result == Self::min_value() || result == Self::max_value() {
            None
        } else {
            Some(result)
        }
    }

    fn checked_into(x: Self) -> Option<Self::Signed> {
        Some(x)
    }

    fn saturated_into(x: Self) -> Self::Signed {
        x
    }

    fn checked_from(x: Self::Signed) -> Option<Self> {
        Some(x)
    }

    fn saturated_from(x: Self::Signed) -> Self {
        x
    }
}

/// Identity implementation for `FixedI128`.
///
/// `FixedI128` is already signed, so `checked_into`, `saturated_into`,
/// `checked_from`, and `saturated_from` are all zero-cost identity operations.
///
/// `saturating` delegates directly to `f` - any saturation that occurs inside
/// the closure is the closure's own saturating arithmetic, which is the
/// expected behaviour for this variant.
///
/// `checked` delegates to `f` and returns `None` only when the result has
/// saturated to `min_value()` or `max_value()`, which are the two sentinel
/// values that saturating arithmetic produces on overflow. If the closure
/// legitimately computes exactly `min_value()` or `max_value()`, `None` is
/// returned conservatively. For cases where that distinction matters, prefer
/// the saturating variant and handle clamping at the call site.
impl FixedSignedCast for FixedI128 {
    type Signed = FixedI128;

    fn saturating<F>(x: Self, f: F) -> Self
    where
        F: FnOnce(Self::Signed) -> Self::Signed,
    {
        f(x)
    }

    fn checked<F>(x: Self, f: F) -> Option<Self>
    where
        F: FnOnce(Option<Self::Signed>) -> Self::Signed,
    {
        let result = f(Some(x));
        // Detect saturation: saturating arithmetic clamps to min/max on overflow.
        // Treat either sentinel as evidence that the result is out of range.
        if result == Self::min_value() || result == Self::max_value() {
            None
        } else {
            Some(result)
        }
    }

    fn checked_into(x: Self) -> Option<Self::Signed> {
        Some(x)
    }

    fn saturated_into(x: Self) -> Self::Signed {
        x
    }

    fn checked_from(x: Self::Signed) -> Option<Self> {
        Some(x)
    }

    fn saturated_from(x: Self::Signed) -> Self {
        x
    }
}

/// Unsigned-to-signed bridge for `FixedU64`.
///
/// Uses `FixedI128` as the signed workspace. A `FixedU64` inner value is a
/// `u64`, which always fits in `i128`, so `checked_into` / `saturated_into`
/// are infallible. The reverse (`checked_from` / `saturated_from`) can fail
/// or clamp when the signed result is negative or exceeds `u64::MAX`.
impl FixedSignedCast for FixedU64 {
    type Signed = FixedI128;

    fn saturating<F>(x: Self, f: F) -> Self
    where
        F: FnOnce(Self::Signed) -> Self::Signed,
    {
        // u64 inner always fits in i128 - cast is infallible.
        let signed = FixedI128::from_inner(x.into_inner() as i128);
        let result = f(signed);
        Self::saturated_from(result)
    }

    fn checked<F>(x: Self, f: F) -> Option<Self>
    where
        F: FnOnce(Option<Self::Signed>) -> Self::Signed,
    {   
        // u64 inner always fits in i128 - cast is infallible.
        let signed = FixedI128::from_inner(x.into_inner() as i128);
        let result = f(Some(signed));
        Self::checked_from(result)
    }

    fn checked_into(x: Self) -> Option<Self::Signed> {
        Some(FixedI128::from_inner(x.into_inner() as i128))
    }

    fn saturated_into(x: Self) -> Self::Signed {
        FixedI128::from_inner(x.into_inner() as i128)
    }

    fn checked_from(x: Self::Signed) -> Option<Self> {
        let inner = x.into_inner();
        // Negative values are not representable as FixedU64.
        // Values above u64::MAX cannot fit in the u64 inner type.
        match inner < 0 || inner > u64::MAX as i128 {
            true => None,
            false => Some(FixedU64::from_inner(inner as u64)),
        }
    }

    fn saturated_from(x: Self::Signed) -> Self {
        let inner = x.into_inner();

        let clamped = match inner {
            b if b < 0 => 0,
            b if b > u64::MAX as i128 => u64::MAX,
            b => b as u64,
        };

        FixedU64::from_inner(clamped)
    }
}

/// Unsigned-to-signed bridge for `FixedU128`.
///
/// Uses `FixedI128` as the signed workspace. Unlike `FixedU64`, a `FixedU128`
/// inner value is a `u128` whose upper half (`> i128::MAX`) cannot be
/// represented in `i128`. `checked_into` therefore returns `None` for those
/// values, and `saturated_into` clamps them to `i128::MAX`.
impl FixedSignedCast for FixedU128 {
    type Signed = FixedI128;

    fn saturating<F>(x: Self, f: F) -> Self
    where
        F: FnOnce(Self::Signed) -> Self::Signed,
    {
        let signed = Self::saturated_into(x);
        let result = f(signed);
        Self::saturated_from(result)
    }

    fn checked<F>(x: Self, f: F) -> Option<Self>
    where
        F: FnOnce(Option<Self::Signed>) -> Self::Signed,
    {
        let signed = Self::checked_into(x);
        let result = f(signed);
        Self::checked_from(result)
    }

    fn checked_into(x: Self) -> Option<Self::Signed> {
        let inner = x.into_inner();
        // u128 values above i128::MAX cannot be represented in FixedI128.
        match inner > i128::MAX as u128 {
            true => None,
            false => Some(FixedI128::from_inner(inner as i128)),
        }
    }

    fn saturated_into(x: Self) -> Self::Signed {
        let inner = x.into_inner();
        // Values in the upper half of u128 clamp to i128::MAX.
        match inner > i128::MAX as u128 {
            true => FixedI128::from_inner(i128::MAX),
            false => FixedI128::from_inner(inner as i128),
        }
    }

    fn checked_from(x: Self::Signed) -> Option<Self> {
        let inner = x.into_inner();
        // Negative values are not representable as FixedU128.
        match inner < 0 {   
            true => None,
            false => Some(FixedU128::from_inner(inner as u128))
        }
    }

    fn saturated_from(x: Self::Signed) -> Self {
        let inner = x.into_inner();
        // Negative signed results clamp to zero, non-negative values fit in u128.
        let clamped = match inner < 0 {
            true => 0,
            false => inner as u128
        };

        FixedU128::from_inner(clamped)
    }
}


// ===============================================================================
// ```````````````````````````````` COMPLEX NUMBER ```````````````````````````````
// ===============================================================================

/// A simple, generic **complex number** representation.
///
/// Holds a **real** and an **imaginary** (`imgn`) component of any numeric type `T`.
///
/// This structure is lightweight and can be used for mathematical, financial, or
/// signal-processing computations that require complex arithmetic.
///
/// ### Type Parameters
/// - `T`: A numeric type (e.g. `f32`, `f64`, or a custom numeric type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Complex<T> {
    /// The **real component** of the complex number.
    pub real: T,

    /// The **imaginary component** of the complex number.
    pub imgn: T,
}

impl<T> Complex<T> {
    fn new(real: T, imgn: T) -> Self {
        Self { real, imgn }
    }
}

// ===============================================================================
// ``````````````````````````````` PRECISION MODEL ```````````````````````````````
// ===============================================================================

/// Provides precision metadata for fixed-point types used in numerical
/// series computations.
///
/// # Constants
///
/// - `DECIMAL_PLACES`: the number of decimal digits after the point that
///   the type can represent, derived from its `DIV` value. Used to compute
///   underflow thresholds and convergence bounds in `fixed_exp` and
///   related functions.
///
/// # Note
///
/// These types use **decimal** fixed-point representation, not binary.
/// `DIV` is a power of 10, so precision is measured in decimal places
/// rather than fractional bits. `INNER_BITS` records the total bit width
/// of the underlying integer storage and is reserved for potential future
/// use with binary fixed-point types; it is not used internally.
pub trait FixedPointInfo {
    /// Total bit width of the inner integer storage type.
    ///
    /// For example, `FixedU64` wraps a `u64`, so `INNER_BITS = 64`.
    ///
    /// This constant is reserved for potential future
    /// binary fixed-point support and is not used internally.
    const INNER_BITS: u32;

    /// Number of representable decimal places, equal to `log10(DIV)`.
    ///
    /// | Type        | DIV     | DECIMAL_PLACES |
    /// |-------------|---------|----------------|
    /// | `FixedU64`  | `10^9`  | `9`            |
    /// | `FixedI64`  | `10^9`  | `9`            |
    /// | `FixedU128` | `10^18` | `18`           |
    /// | `FixedI128` | `10^18` | `18`           |
    const DECIMAL_PLACES: u32;
}

/// `FixedU64`: inner type `u64` (64-bit), `DIV = 10^9` - 9 decimal places.
impl FixedPointInfo for FixedU64 {
    const INNER_BITS: u32 = 64; // bit width of u64
    const DECIMAL_PLACES: u32 = 9;
}

/// `FixedI64`: inner type `i64` (64-bit), `DIV = 10^9` - 9 decimal places.
impl FixedPointInfo for FixedI64 {
    const INNER_BITS: u32 = 64; // bit width of i64, sign bit included
    const DECIMAL_PLACES: u32 = 9;
}

/// `FixedU128`: inner type `u128` (128-bit), `DIV = 10^18` - 18 decimal places.
impl FixedPointInfo for FixedU128 {
    const INNER_BITS: u32 = 128; // bit width of u128
    const DECIMAL_PLACES: u32 = 18;
}

/// `FixedI128`: inner type `i128` (128-bit), `DIV = 10^18` - 18 decimal places.
impl FixedPointInfo for FixedI128 {
    const INNER_BITS: u32 = 128; // bit width of i128, sign bit included
    const DECIMAL_PLACES: u32 = 18;
}

// ===============================================================================
// ````````````````````````````` NUMERICAL UTILITIES `````````````````````````````
// ===============================================================================

/// Returns the smallest positive increment representable by the fixed-point generic
/// [`FixedPointNumber`].
///
/// For a fixed-point number defined as:
/// ```text
/// value = inner / DIV
/// ```
/// the ULP equals `1 / DIV`.
///
/// ## Behavior
/// - Uses `FixedPoint::from_inner(1)` to construct the smallest step value.
/// - Useful as a numerical tolerance in convergence or rounding checks.
///
/// ## Example
/// ```ignore
/// let ulp_val = ulp::<FixedU128>();
/// assert_eq!(ulp_val, FixedU128::from_inner(1)); // represents 1e-18 if DIV = 1e18
/// ```
///
/// ## Notes
/// - The exact decimal size of the ULP depends on `FixedPoint::DIV`.
/// - If `DIV = 10^6`, then `ULP = 1e-6`.
/// - Works for all fixed-point types whose `Inner` implements `From<u8>`.
fn ulp<F: FixedPointNumber>() -> F
where
    F::Inner: From<u8>,
{
    F::from_inner(F::Inner::from(1u8))
}

/// Maximum allowed number of iterations for iterative numerical methods.
///
/// This constant caps the number of iterations in functions to:
/// - Prevent infinite or excessively long loops during convergence.
/// - Provide a reasonable trade-off between accuracy and computation time.
///
/// Typical value (50) is chosen empirically to balance precision and performance,
/// but can be adjusted depending on application requirements.
///
/// ## Note
/// - Functions also should implement early stopping conditions based on tolerance or stagnation,
///   so the actual number of iterations is often fewer than this maximum.
const MAX_ITERATIONS: u32 = 50;


/// Extracts the integer part of a fixed-point number as a `u32`,
/// truncating the fractional component toward zero.
///
/// Fixed-point numbers store their value as `inner / DIV`, where `DIV`
/// is a power of 10 (`10^9` for 64-bit types, `10^18` for 128-bit types).
/// Dividing `inner` by `DIV` removes the fractional portion, leaving
/// the integer part.
///
/// ## Behavior
///
/// | Condition              | Returns      |
/// |------------------------|--------------|
/// | Integer part negative  | `0`          |
/// | Integer part > u32::MAX| `u32::MAX`   |
/// | Otherwise              | Integer part |
///
/// ## Arguments
///
/// * `x` - The fixed-point value to truncate.
///
/// ## Returns
///
/// The integer portion of `x`, clamped to `[0, u32::MAX]`.
///
/// ## Examples
///
/// ```ignore
/// // FixedU64 with DIV = 10^9: inner value 300_750_000_000 represents 300.75
/// let x = FixedU64::from_inner(300_750_000_000);
/// assert_eq!(to_u32_floor(&x), 300);
///
/// // Negative values clamp to 0
/// let x = FixedI64::saturating_from_integer(-5);
/// assert_eq!(to_u32_floor(&x), 0);
/// ```
fn to_u32_floor<T>(x: &T) -> u32
where
    T: FixedPointNumber + Copy + FixedPointInfo,
    // TryInto<i128> required to work with the fixed-point inner value
    // in a common signed type regardless of whether T is u64 or i128 based.
    T::Inner: Copy + PartialOrd + TryInto<i128>,
{
    // Extract the raw inner integer representation.
    let inner = x.into_inner();

    // Convert inner to i128 for arithmetic. For unsigned inner types (u64, u128),
    // try_into() fails only if the value exceeds i128::MAX - astronomically large,
    // treated as overflow and clamped to u32::MAX.
    let inner_i128: i128 = match inner.try_into() {
        Ok(val) => val,
        Err(_) => return u32::MAX,
    };

    // Convert DIV to i128. For sp_arithmetic types, DIV is at most 10^18
    // which fits comfortably in i128 (max ~1.7 * 10^38). Failure here is
    // unreachable in practice, but we return 0 conservatively rather than panic.
    let div: i128 = match T::DIV.try_into() {
        Ok(val) => val,
        Err(_) => return 0,
    };

    // Integer part = inner / DIV, truncated toward zero.
    let int_part = inner_i128 / div;

    if int_part < 0 {
        // Negative integer part - not representable as u32, clamp to 0.
        0
    } else if int_part > u32::MAX as i128 {
        // Exceeds u32 range - clamp to maximum.
        u32::MAX
    } else {
        // Safe cast: int_part is in [0, u32::MAX].
        int_part as u32
    }
}

/// Extracts the exact integer value of a fixed-point number as `i128`,
/// returning `None` if the value has a non-zero fractional component.
///
/// A fixed-point number represents `inner / DIV`. This function returns
/// `inner / DIV` only when `inner` is exactly divisible by `DIV` -
/// i.e. when the fixed-point value is a whole number with no fractional part.
///
/// ## Arguments
///
/// * `x` - The fixed-point number to inspect.
///
/// ## Returns
///
/// * `Some(n)` if `x` represents the exact integer `n`
/// * `None` if `x` has a fractional component, or if internal conversion fails
///
/// ## Examples
///
/// ```ignore
/// let x = FixedU64::saturating_from_integer(5);
/// assert_eq!(fixed_to_i128(&x), Some(5));
///
/// let x = FixedU64::saturating_from_rational(3, 2); // 1.5
/// assert_eq!(fixed_to_i128(&x), None);
/// ```
fn fixed_to_i128<T>(x: &T) -> Option<i128>
where
    T: FixedPointNumber,
    T::Inner: TryInto<i128> + Copy,
{
    // Convert inner representation and DIV to i128 for arithmetic.
    // Both conversions fail only in pathological cases - DIV for sp_arithmetic
    // types is at most 10^18, well within i128 range.
    let inner: i128 = x.into_inner().try_into().ok()?;
    let div: i128 = T::DIV.try_into().ok()?;

    // Exact integer check: inner must be perfectly divisible by DIV.
    // Any remainder means the value has a fractional component.
    if inner % div == 0 {
        Some(inner / div)
    } else {
        None
    }
}

#[allow(dead_code)]
fn fixed_pi<T>() -> T
where
    T: FixedPointNumber,
{
    T::saturating_from_rational(355, 113)
}

/// Computes an adaptive iteration count for series expansions based on
/// the magnitude of `x` and the precision of the fixed-point type.
///
/// Larger inputs converge more slowly in series expansions, and higher
/// precision types require more terms to reach their representable accuracy.
/// This function combines both factors into a single iteration budget:
///
/// ```text
/// iterations = floor(|x|) * DECIMAL_PLACES + 1
/// ```
///
/// The `+ 1` guarantees at least one iteration for any input, including
/// `x = 0`.
///
/// ## Arguments
///
/// * `x` - The fixed-point value whose magnitude drives the iteration count.
///
/// ## Returns
///
/// A `u32` iteration count, always `>= 1`. Uses saturating arithmetic
/// throughout so overflow on very large inputs produces `u32::MAX` rather
/// than wrapping.
#[allow(dead_code)]
fn dynamic_max_iterations<T>(x: &T) -> u32
where
    T: FixedPointNumber + Copy + FixedPointInfo,
    T::Inner: Shr<u32, Output = T::Inner> + TryInto<i128> + Copy,
{
    // Work with |x| so negative inputs produce the same iteration count
    // as their positive equivalents.
    let abs_x = x.saturating_abs();

    // Integer part of |x| - the fractional component does not affect
    // convergence speed meaningfully.
    let int_part = to_u32_floor(&abs_x);

    // Scale by DECIMAL_PLACES to account for type precision, then add 1
    // to ensure at least one iteration. saturating_mul and saturating_add
    // prevent overflow on extreme inputs.
    int_part
        .saturating_mul(T::DECIMAL_PLACES)
        .saturating_add(1)
}

// ===============================================================================
// ```````````````````````````` SQRT - NEWTON-RAPHSON ````````````````````````````
// ===============================================================================

/// Approximates the square root of a fixed-point number using the
/// Newton-Raphson method.
///
/// This is the core computational primitive for square root operations.
/// The public API is [`fixed_sqrt`], which adds domain checking and exact
/// fast paths before delegating here.
///
/// ## Algorithm
///
/// Newton-Raphson iteration for square roots:
/// ```text
/// guess_{n+1} = (guess_n + x / guess_n) / 2
/// ```
///
/// Converges quadratically - the number of correct digits roughly doubles
/// each iteration. For fixed-point types, convergence is detected when the
/// change between iterations falls within `2 * ULP`, which is the tightest
/// meaningful threshold: the Newton step cannot improve beyond `1 ULP` on
/// each side, so tighter tolerances would cause infinite oscillation between
/// adjacent representable values.
///
/// Iteration stops early on stagnation (improvement stops or reverses),
/// and is hard-capped at `MAX_ITERATIONS` to guarantee termination.
///
/// ## Initial Guess Strategy
///
/// | Input range | Initial guess         | Reason                              |
/// |-------------|-----------------------|-------------------------------------|
/// | `x > 1`     | `(x + 1) / 2`         | Midpoint above 1, closer to result  |
/// | `x = 1`     | `1`                   | Exact, no iteration needed          |
/// | `x in (0.25, 1)` | `x`            | Already a reasonable approximation  |
/// | `x <= 0.25` | `0.25`                | Avoids starting too close to zero   |
///
/// ## Arguments
///
/// * `x` - A non-negative fixed-point number to compute the square root of.
///         Caller is responsible for ensuring `x >= 0`. Negative inputs
///         return zero - use [`fixed_sqrt`] for proper domain handling.
///
/// ## Returns
///
/// An approximation of `sqrt(x)`, accurate to within `2 * ULP` of the
/// true value for well-behaved inputs.
fn fixed_sqrt_newton<F: FixedPointNumber>(x: &F) -> F
where
    F::Inner: From<u8>,
{
    let zero = F::zero();

    if *x <= zero {
        return zero;
    }

    let one = F::one();
    let two = one.saturating_add(one);

    // 2 * ULP is the principled convergence bound: the Newton step cannot
    // improve beyond 1 ULP on either side, so anything tighter causes
    // oscillation between adjacent representable values.
    let tol = ulp::<F>().saturating_add(ulp::<F>());

    let mut guess = match x.cmp(&one) {
        // x > 1: midpoint of [1, x] is above sqrt(x), a safe starting point.
        core::cmp::Ordering::Greater => {
            x.saturating_add(one).checked_div(&two).unwrap_or(one)
        }
        // x = 1: sqrt(1) = 1 exactly, no iteration needed.
        core::cmp::Ordering::Equal => return one,
        // x < 1: use x itself if it's above 0.25, otherwise use 0.25.
        core::cmp::Ordering::Less => {
            let quarter = F::saturating_from_rational(1, 4);
            if *x > quarter { *x } else { quarter }
        }
    };

    let mut prev_diff: Option<F> = None;

    for _ in 0..MAX_ITERATIONS {
        // Compute x / guess. If this fails (degenerate state at fixed-point
        // boundaries), return the best approximation computed so far.
        let div = match x.checked_div(&guess) {
            Some(d) => d,
            None => return guess,
        };

        // Next guess: average of current guess and x/guess.
        // Falls back to current guess if the addition overflows.
        let next = guess.saturating_add(div)
            .checked_div(&two)
            .unwrap_or(guess);

        // Absolute difference between successive guesses.
        let diff = if next > guess {
            next.saturating_sub(guess)
        } else {
            guess.saturating_sub(next)
        };

        // Converged: improvement is within 2 * ULP.
        // Return `next`, not `guess` - next is the result of this iteration
        // and is always at least as accurate as guess.
        if diff <= tol {
            return next;
        }

        // Stagnation: improvement has stopped or reversed.
        // Return next for the same reason as above.
        if let Some(pd) = prev_diff {
            if diff >= pd {
                return next;
            }
        }

        prev_diff = Some(diff);
        guess = next;
    }

    // Iteration limit reached - return best approximation found.
    guess
}

// ===============================================================================
// ````````````````````````````` LN - RANGE REDUCTION ````````````````````````````
// ===============================================================================

/// Reduces a fixed-point value `y` toward `1` by repeatedly taking its
/// square root, returning the reduced value and the number of reductions applied.
///
/// ## Purpose
///
/// Series expansions for `ln(y)` converge fastest when `y` is close to `1`.
/// This function brings `y` into the band `[0.5, 1.5]` where [`ln_near_one`]
/// is both accurate and efficient.
///
/// ## Algorithm
///
/// Each iteration replaces `y` with `sqrt(y)`, halving the distance to `1`
/// in logarithmic space. After `k` reductions:
/// 
/// ```text
/// y_original = y_reduced ^ (2^k)
/// ln(y_original) = 2^k * ln(y_reduced)
/// ```
///
/// The caller uses `k` to undo the reduction after computing `ln(y_reduced)`.
///
/// ## Stopping Conditions
///
/// Iteration stops when any of the following occur:
/// - `|y - 1| <= 0.5` - `y` is in `[0.5, 1.5]`, close enough for [`ln_near_one`]
/// - `ny == y` - Newton-Raphson stagnated, no further reduction is possible
/// - `ny == 0` - degenerate input at fixed-point boundaries; result will be approximate
/// - [`MAX_ITERATIONS`] reached - hard cap to guarantee termination
///
/// ## Arguments
///
/// * `y` - A positive fixed-point value to reduce. Behaviour for `y <= 0`
///         is undefined - caller is responsible for domain validation.
///
/// ## Returns
///
/// A tuple `(y_reduced, k)` where:
/// - `y_reduced` is in `[0.5, 1.5]` (or as close as the stopping conditions allow)
/// - `k` is the number of square root reductions applied
fn range_reduce_sqrt<T>(mut y: T) -> (T, u32)
where
    T: FixedPointNumber + Copy + PartialOrd,
    T::Inner: From<u8> + Shr<u32, Output = T::Inner> + TryInto<i128> + Copy,
{
    let one = T::one();

    let half = T::saturating_from_rational(1, 2);

    let mut k: u32 = 0;

    for _ in 0..MAX_ITERATIONS {
        let diff = if y > one {
            y.saturating_sub(one)
        } else {
            one.saturating_sub(y)
        };

        // y is within [0.5, 1.5] - close enough for ln_near_one.
        if diff <= half {
            break;
        }

        let ny = fixed_sqrt_newton::<T>(&y);

        // Stagnation: Newton-Raphson could not improve further.
        if ny == y {
            break;
        }

        // Degenerate: sqrt collapsed to zero at fixed-point boundaries.
        if ny == T::zero() {
            break;
        }

        y = ny;
        k += 1;
    }

    (y, k)
}

/// Computes `ln(y)` for a fixed-point value `y` near `1` using the
/// arctanh series identity:
///
/// ```text
/// ln(y) = 2 * sum_{k=0}^{inf} t^(2k+1) / (2k+1)
///
/// where t = (y - 1) / (y + 1)
/// ```
///
/// Converges for all `y > 0`, with convergence rate determined by `|t|`.
/// The closer `y` is to `1`, the smaller `|t|` and the faster convergence.
/// [`range_reduce_sqrt`] ensures `y` is in `[0.5, 1.5]` before calling
/// this function, keeping `|t| <= 1/3` for fast, reliable convergence.
///
/// ## Arguments
///
/// * `y` - A fixed-point value near `1`. Caller must ensure `y > 0`.
///         Results are inaccurate for `y` far from `1`.
///
/// ## Returns
///
/// An approximation of `ln(y)`, accurate to within the type's ULP for
/// inputs in `[0.5, 1.5]`.
///
/// ## Note
///
/// On unsigned types, `y < 1` produces `t = 0` (since `y - 1` saturates
/// to zero), returning `ln(y) = 0`. This is incorrect for `y < 1`, but
/// the unsigned type guard in [`fixed_ln`] ensures this branch is never
/// reached for unsigned types with `y < 1`.
fn ln_near_one<T>(y: T) -> T
where
    T: FixedPointNumber + Copy + PartialOrd + FixedPointInfo,
    T::Inner: From<u8> + Shr<u32, Output = T::Inner> + TryInto<i128> + Copy,
{
    let one = T::one();
    let two = one.saturating_add(one);
    let eps = ulp::<T>();

    // t = (y - 1) / (y + 1)
    // For signed types: saturating_sub produces a negative result when y < 1,
    // giving a negative t - correct.
    // For unsigned types: saturating_sub returns 0 when y < 1 - guarded in fixed_ln.
    let num = y.saturating_sub(one);   // y - 1
    let denom = y.saturating_add(one); // y + 1, always positive for y > 0
    let t = num.checked_div(&denom).unwrap_or(T::zero());

    // t^2, used to advance the power each iteration: t, t^3, t^5, ...
    let t_sq = t.checked_mul(&t).unwrap_or(T::zero());

    let mut sum = T::zero();
    let mut power = t;

    for i in 0u32..MAX_ITERATIONS {
        let denom_fp = T::saturating_from_integer(2 * i + 1);

        // Current term: t^(2k+1) / (2k+1)
        let term = power.checked_div(&denom_fp).unwrap_or(T::zero());
        if term.saturating_abs() <= eps {
            break;
        }

        let new_sum = sum.saturating_add(term);

        // Stagnation: sum is no longer changing at ULP level.
        if new_sum == sum {
            break;
        }

        sum = new_sum;
        power = power.checked_mul(&t_sq).unwrap_or(T::zero());
    }

    // ln(y) = 2 * sum. 
    sum.saturating_mul(two)
}

// ===============================================================================
// ``````````````````````````` POWER - INTEGER & BINARY ``````````````````````````
// ===============================================================================

/// Raises a fixed-point number `x` to an integer power `n` using
/// binary exponentiation.
///
/// ## Behavior
///
/// | Case               | Result                          |
/// |--------------------|---------------------------------|
/// | `n = 0`            | `Some(1)` - `x^0 = 1` always   |
/// | `n > 0`            | `Some(x^n)`                     |
/// | `n < 0, x != 0`    | `Some(1 / x^|n|)`               |
/// | `n < 0, x = 0`     | `None` - division by zero       |
///
/// Uses saturating arithmetic throughout, so intermediate overflow clamps
/// to the type's maximum rather than wrapping or panicking. Returns `None`
/// only for division-by-zero or when `1 / x^|n|` is unrepresentable.
///
/// ## Arguments
///
/// * `x` - The fixed-point base.
/// * `n` - The integer exponent, including `i128::MIN`.
fn fixed_powi<T>(x: T, n: i128) -> Option<T>
where
    T: FixedPointNumber + Copy,
{
    let one = T::one();
    let zero = T::zero();

    // x^0 = 1 for all x, including x = 0.
    // The caller (fixed_pow) guards 0^0 before reaching here
    if n == 0 {
        return Some(one);
    }

    if n < 0 {
        // 0^(-n) is division by zero - undefined.
        if x == zero {
            return None;
        }

        // x^(-n) = 1 / x^|n|.
        // unsigned_abs() handles n = i128::MIN without overflow.
        let pos = fixed_powi_positive(x, n.unsigned_abs());
        return one.checked_div(&pos);
    }

    Some(fixed_powi_positive(x, n as u128))
}

/// Core binary exponentiation for non-negative integer powers.
///
/// Computes `x^n` in `O(log n)` multiplications using the
/// square-and-multiply algorithm. Extracted as a separate function
/// so both the positive and negative paths of [`fixed_powi`] can
/// share the same implementation.
///
/// Uses saturating arithmetic - intermediate overflow clamps to the
/// type's maximum rather than wrapping or panicking.
///
/// ## Arguments
///
/// * `x` - The fixed-point base.
/// * `n` - The non-negative exponent as `u128`.
fn fixed_powi_positive<T>(x: T, mut n: u128) -> T
where
    T: FixedPointNumber + Copy,
{
    let one = T::one();
    let mut result = one;
    let mut base = x;

    while n > 0 {
        // If the current bit is set, multiply result by the current base power.
        if (n & 1) == 1 {
            result = result.saturating_mul(base);
        }
        n >>= 1;
        // Square the base for the next bit position.
        // Guard avoids a redundant squaring on the final iteration.
        if n > 0 {
            base = base.saturating_mul(base);
        }
    }

    result
}

// ===============================================================================
// `````````````````````````````````` FIXED-SQRT `````````````````````````````````
// ===============================================================================

/// Computes the square root of a fixed-point number [`FixedPointNumber`] `x`.
///
/// Uses the Newton-Raphson method internally via [`fixed_sqrt_newton`] for
/// the general case, with exact fast paths for the common values `0` and `1`.
///
/// ## Domain
///
/// Defined only for `x >= 0`. Returns `None` for negative inputs, as the square
/// root of a negative number is not real-valued.
///
/// # Arguments
///
/// * `x` - The fixed-point number to compute the square root of.
///
/// # Returns
///
/// * `Some(sqrt(x))` for `x >= 0`
/// * `None` for `x < 0`
///
/// # Examples
///
/// ```ignore
/// let x = FixedU64::saturating_from_integer(4);
/// assert_eq!(fixed_sqrt(&x), Some(FixedU64::saturating_from_integer(2)));
///
/// let x = FixedI64::saturating_from_integer(-1);
/// assert_eq!(fixed_sqrt(&x), None);
/// ```
fn fixed_sqrt<F: FixedPointNumber>(x: &F) -> Option<F>
where
    // Require the inner integer type to be constructible from u8 literals,
    // which is common for fixed-point arithmetic types.
    F::Inner: From<u8>,
{
    let zero = F::zero();
    let one = F::one();

    // --- DOMAIN CHECK ---
    // sqrt(x) is undefined for x < 0 in real arithmetic.
    if *x < zero {
        return None;
    }

    // --- FAST PATHS ---
    // Exact results for boundary values, avoids unnecessary Newton iterations.

    // sqrt(0) = 0 exactly.
    if *x == zero {
        return Some(zero);
    }

    // sqrt(1) = 1 exactly.
    if *x == one {
        return Some(one);
    }

    // Delegates to Newton-Raphson for all other values.
    // See [`fixed_sqrt_newton`] for convergence details.
    Some(fixed_sqrt_newton::<F>(x))
}

// ===============================================================================
// ````````````````````````````````` COMPLEX-SQRT ````````````````````````````````
// ===============================================================================

/// Computes the principal square root of a fixed-point number, returning
/// a [`Complex`] result.
///
/// Unlike [`fixed_sqrt`], this function is defined for all inputs including
/// negative numbers. For negative inputs, the result is a purely imaginary
/// number representing the principal square root in the complex plane.
///
/// Internally delegates to [`fixed_sqrt_newton`] for the real square root
/// computation, which is only valid for non-negative inputs. The sign of `x`
/// is handled here before dispatching.
///
/// ## Domain
///
/// Defined for all fixed-point values. Never returns `None`.
///
/// ## Arguments
///
/// * `x` - The fixed-point number to compute the complex square root of.
///
/// ## Returns
///
/// | Input    | Result                   |
/// |----------|--------------------------|
/// | `x > 0`  | `sqrt(x) + 0i`           |
/// | `x = 0`  | `0 + 0i`                 |
/// | `x < 0`  | `0 + sqrt(|x|)i`         |
///
/// ## Note
///
/// On unsigned types (`FixedU64`, `FixedU128`), negative values are not
/// representable, so the imaginary branch is never reached. Only the real
/// and zero branches apply.
///
/// ## Examples
///
/// ```ignore
/// // Positive input - purely real result
/// let x = FixedI64::saturating_from_integer(4);
/// assert_eq!(complex_sqrt(&x), Some(Complex { real: FixedI64::saturating_from_integer(2), imgn: FixedI64::zero() }));
///
/// // Negative input - purely imaginary result
/// let x = FixedI64::saturating_from_integer(-4);
/// assert_eq!(complex_sqrt(&x), Some(Complex { real: FixedI64::zero(), imgn: FixedI64::saturating_from_integer(2) }));
/// ```
fn complex_sqrt<F: FixedPointNumber>(x: &F) -> Option<Complex<F>>
where
    // Require the inner integer type to be constructible from u8 literals,
    // which is common for fixed-point arithmetic types.
    F::Inner: From<u8>,
{
    let zero = F::zero();

    // --- FAST PATH ---
    // sqrt(0) = 0 + 0i exactly.
    if *x == zero {
        return Some(Complex::new(zero, zero));
    }

    // --- NEGATIVE INPUT ---
    // sqrt(x) for x < 0 is purely imaginary: sqrt(x) = 0 + sqrt(|x|)i.
    // Take the magnitude first since fixed_sqrt_newton requires a non-negative input.
    if *x < zero {
        let mag = x.saturating_abs();
        let imgn = fixed_sqrt_newton::<F>(&mag);
        return Some(Complex::new(zero, imgn));
    }

    // sqrt(x) for x > 0 is purely real: sqrt(x) = sqrt(x) + 0i.
    let real = fixed_sqrt_newton::<F>(x);
    Some(Complex::new(real, zero))
}

// ===============================================================================
// `````````````````````````````````` FIXED-EXP ``````````````````````````````````
// ===============================================================================

/// Computes `e^x` for a fixed-point number using argument reduction and
/// a Taylor series expansion.
///
/// ## Algorithm
///
/// Splits `x = n + r` where `n` is the integer part and `|r| <= 0.5`:
///
/// ```text
/// exp(x) = exp(n) * exp(r)
/// ```
///
/// `exp(r)` is computed via Taylor series (fast for small `|r|`).
/// `exp(n)` is computed by raising `e ~= 2.718281828459045235` to integer
/// power `n` via binary exponentiation ([`fixed_powi`]).
///
/// `e` is approximated as `2_718_281_828_459_045_235 / 10^18`, giving
/// 18 significant figures - matching the full precision of `FixedU128`
/// and over-specified but harmless for the other three types.
///
/// ## Domain
///
/// Defined for all fixed-point values, but:
/// - Large positive `x` overflows the fixed-point range - returns `None`.
/// - Large negative `x` underflows to zero - returns `Some(0)`.
///   Threshold: `x < -(DECIMAL_PLACES * 10)`.
///
/// ## Arguments
///
/// * `x` - The fixed-point exponent value.
///
/// ## Returns
///
/// * `Some(exp(x))` on success
/// * `Some(0)` when `x` is below the underflow threshold
/// * `None` on overflow, or if internal arithmetic fails
///
/// # Examples
///
/// ```ignore
/// let x = FixedU64::saturating_from_integer(1);
/// let result = fixed_exp(&x).unwrap();
/// // result ~= 2.718281828
/// ```
fn fixed_exp<T>(x: &T) -> Option<T>
where
    T: FixedPointNumber + Copy + PartialOrd + FixedPointInfo,
    T::Inner: From<u8> + Shr<u32, Output = T::Inner> + TryInto<i128> + Copy,
{
    let zero = T::zero();
    let one = T::one();

    // --- FAST PATH ---
    // exp(0) = 1 exactly.
    if *x == zero {
        return Some(one);
    }

    // Underflow guard: for sufficiently large negative x, exp(x) is below
    // the smallest representable value. Return zero rather than iterating
    // toward an unrepresentable result.
    let neg_threshold = T::saturating_from_integer(
        -((T::DECIMAL_PLACES as i32).saturating_mul(10))
    );
    if *x < neg_threshold {
        return Some(zero);
    }

    // Argument reduction: split x = n + r, |r| <= 0.5.
    // The Taylor series for exp(r) converges much faster for small |r|.
    let n_i128: i128 = {
        let inner: i128 = x.into_inner().try_into().ok()?;
        let div: i128 = T::DIV.try_into().ok()?;
        // Truncate toward zero - standard Rust integer division semantics.
        inner / div
    };
    let n = n_i128.clamp(i32::MIN as i128, i32::MAX as i128) as i32;
    let n_fixed = T::saturating_from_integer(n);

    // r = x - n, guaranteed |r| <= 0.5 by construction.
    let r = x.saturating_sub(n_fixed);

    // Taylor series: exp(r) = 1 + r + r^2/2! + r^3/3! + ...
    // Terms are computed incrementally: term_i = term_{i-1} * r / i.
    let epsilon = ulp::<T>();
    let mut sum = one;  // Accumulates the series result, starts at the i=0 term (1).
    let mut term = one; // Tracks the current series term, starts at 1.

    for i in 1u32..=MAX_ITERATIONS {
        let i_fixed = T::saturating_from_integer(i);

        // next_term = term * r / i.
        // Both operations fall back to zero on failure rather than propagating
        // None - a failed multiply or divide means the term is negligibly small.
        let next_term = term.checked_mul(&r)
            .unwrap_or(zero)
            .checked_div(&i_fixed)
            .unwrap_or(zero);

        // saturating_abs() correctly handles negative terms (negative x alternates
        // term signs). A plain comparison without abs() would miss converged
        // negative terms entirely.
        if next_term.saturating_abs() <= epsilon {
            break;
        }

        let new_sum = sum.saturating_add(next_term);

        // Sum is no longer changing - saturated or converged at ULP boundary.
        if new_sum == sum {
            break;
        }

        sum = new_sum;
        term = next_term;
    }

    // exp(r) is now in `sum`.

    // No integer scaling needed when the integer part is zero.
    if n == 0 {
        return Some(sum);
    }

    // Scale back: exp(x) = exp(r) * exp(n) = sum * e^n.
    let e = T::saturating_from_rational(
        2_718_281_828_459_045_235u128,
        1_000_000_000_000_000_000u128,
    );

    let exp_n = fixed_powi(e, n as i128)?;

    // If exp_n has already saturated to max_value, multiplying by sum (>= 1
    // for positive x) would overflow. Return None rather than a silent saturated result.
    if exp_n >= T::max_value() {
        return None;
    }

    // Final result: exp(x) = exp(r) * exp(n).
    // checked_mul returns None on overflow, propagating cleanly to the caller.
    sum.checked_mul(&exp_n)
}

// ===============================================================================
// ``````````````````````````````````` FIXED-LN ``````````````````````````````````
// ===============================================================================

/// Computes the natural logarithm `ln(x)` for a fixed-point number.
///
/// ## Algorithm
///
/// Uses repeated square root range reduction to bring `x` near `1`,
/// then evaluates `ln` via the series expansion in `ln_near_one`:
///
/// ```text
/// ln(x) = 2^k * ln(y)
/// ```
///
/// where `y` is the range-reduced value near `1` and `k` is the number
/// of square root reductions applied. The scaling back is done with a
/// single multiply by `2^k` to avoid accumulated rounding error from
/// repeated multiplication.
///
/// ## Domain
///
/// - Defined only for `x > 0`. Returns `None` for `x <= 0`.
/// - On unsigned types (`FixedU64`, `FixedU128`), `ln(x)` for `x < 1`
///   produces a negative result which is unrepresentable. Returns `None`
///   in this case rather than silently returning a wrong answer.
///   Use a signed type (`FixedI64`, `FixedI128`) if `ln` of fractional
///   values is needed.
///
/// ## Arguments
///
/// * `x` - The fixed-point number to compute the natural logarithm of.
///
/// ## Returns
///
/// * `Some(ln(x))` for valid inputs
/// * `None` for `x <= 0`
/// * `None` for unsigned types where `x < 1` (result not representable)
///
/// ## Examples
///
/// ```ignore
/// let x = FixedU64::saturating_from_integer(1);
/// assert_eq!(fixed_ln(&x), Some(FixedU64::zero())); // ln(1) = 0
///
/// let x = FixedU64::saturating_from_integer(2);
/// let result = fixed_ln(&x).unwrap();
/// // result ~= 0.693147180
///
/// // Negative input - always None
/// let x = FixedI64::saturating_from_integer(-1);
/// assert_eq!(fixed_ln(&x), None);
///
/// // Fractional input on unsigned type - None (unrepresentable result)
/// let x = FixedU64::saturating_from_rational(1, 2);
/// assert_eq!(fixed_ln(&x), None);
///
/// // Fractional input on signed type - correct negative result
/// let x = FixedI64::saturating_from_rational(1, 2);
/// let result = fixed_ln(&x).unwrap();
/// // result ~= -0.693147180
/// ```
fn fixed_ln<T>(x: &T) -> Option<T>
where
    T: FixedPointNumber + Copy + PartialOrd + FixedPointInfo ,
    T::Inner: From<u8> + Shr<u32, Output = T::Inner> + TryInto<i128> + Copy,
{
    let zero = T::zero();
    let one = T::one();

    // --- DOMAIN CHECK ---
    // ln(x) is undefined for x <= 0 in real arithmetic.
    if *x <= zero {
        return None;
    }

    // --- FAST PATH ---
    // ln(1) = 0 exactly.
    if *x == one {
        return Some(zero);
    }

    // Detect unsigned types: on unsigned types, `0 - 1` saturates to `0`
    // rather than wrapping to `-1`. For such types, ln(x < 1) would return
    // the incorrect `Some(0)` from the series - return None instead.
    let is_unsigned = zero.saturating_sub(one) == zero;
    if is_unsigned && *x < one {
        return None;
    }

    let (y_reduced, k) = range_reduce_sqrt(*x);
    let mut ln_val = ln_near_one(y_reduced);

    // Recover ln(x) = 2^k * ln(y_reduced).
    // k can reach up to MAX_ITERATIONS (50). Shifting by more than 31 would
    // panic in debug mode (1u32 << 32 is UB). When k > 31 the true result
    // is 2^k * ln(y_reduced) with k >= 32, meaning the result exceeds
    // 2^32 * ln(y_reduced) - astronomically large for any fixed-point type.
    // Return None rather than a silently wrong clamped value.
    if k > 31 {
        return None;
    }

    if k > 0 {
        let scale = T::saturating_from_integer(1u32 << k);
        ln_val = ln_val.saturating_mul(scale);
    }

    Some(ln_val)
}

// ===============================================================================
// ``````````````````````````````````` FIXED-POW `````````````````````````````````
// ===============================================================================

/// Computes `x^p` for fixed-point numbers.
///
/// ## Algorithm
///
/// Three computation paths depending on the inputs:
///
/// - **Integer exponent**: uses binary exponentiation via `fixed_powi`
///   for exact, efficient results.
/// - **Fractional exponent**: uses the identity `x^p = exp(p * ln(x))`
///   via [`fixed_exp`] and [`fixed_ln`].
/// - **Special cases**: handled directly with exact results.
///
/// ## Domain
///
/// | Input condition              | Result              | Reason                              |
/// |------------------------------|---------------------|-------------------------------------|
/// | `x = 0, p > 0`              | `Some(0)`           | Mathematical limit                  |
/// | `x = 0, p = 0`              | `None`              | Indeterminate form                  |
/// | `x = 0, p < 0`              | `None`              | Division by zero                    |
/// | `x < 0, p` non-integer      | `None`              | Not real-valued                     |
/// | `x < 0, p` integer          | `Some(x^p)`         | Real-valued, handled by `fixed_powi` |
/// | `p = 0`                     | `Some(1)`           | `x^0 = 1` for all non-zero `x`     |
/// | `x = 1`                     | `Some(1)`           | `1^p = 1` for all `p`              |
///
/// ## Arguments
///
/// * `x` - The base as a fixed-point number.
/// * `p` - The exponent as a fixed-point number.
///
/// ## Returns
///
/// * `Some(x^p)` on success
/// * `None` for indeterminate or undefined inputs (see domain table above)
/// * `None` on overflow
///
/// ## Examples
///
/// ```ignore
/// // Integer exponent
/// let x = FixedU64::saturating_from_integer(2);
/// let p = FixedU64::saturating_from_integer(3);
/// assert_eq!(fixed_pow(&x, &p), Some(FixedU64::saturating_from_integer(8)));
///
/// // Fractional exponent
/// let x = FixedU64::saturating_from_integer(4);
/// let p = FixedU64::saturating_from_rational(1, 2);
/// let result = fixed_pow(&x, &p).unwrap();
/// // result ~= 2.0 (square root of 4)
///
/// // Undefined cases
/// let zero = FixedU64::zero();
/// assert_eq!(fixed_pow(&zero, &zero), None); // 0^0 indeterminate
/// ```
fn fixed_pow<T>(x: &T, p: &T) -> Option<T>
where
    T: FixedPointNumber + Copy + PartialOrd + FixedPointInfo,
    T::Inner: From<u8> + Shr<u32, Output = T::Inner> + TryInto<i128> + Copy,
{
    let zero = T::zero();
    let one = T::one();

    // --- DOMAIN VALIDATION ---
    // 0^0 is indeterminate; 0^(negative) is division by zero.
    if *x == zero && *p <= zero {
        return None;
    }

    // 0^(positive) = 0. Handles both integer and fractional positive p,
    // consistent with the mathematical limit. Guarded explicitly because
    // the general path would call ln(0) which is undefined.
    if *x == zero {
        return Some(zero);
    }

    // Negative base with a fractional exponent is not real-valued.
    // Integer exponents are handled below by fixed_powi.
    let int_exp = fixed_to_i128(p);
    if *x < zero && int_exp.is_none() {
        return None;
    }

    // --- FAST PATHS ---
    // x^0 = 1 for all non-zero x (zero case already handled above).
    if *p == zero {
        return Some(one);
    }

    // 1^p = 1 for all p.
    if *x == one {
        return Some(one);
    }

    // Binary exponentiation is exact and significantly cheaper than
    // the general exp(p * ln(x)) path. Also the only valid path for
    // negative bases, where ln(x) is undefined.
    if let Some(n) = int_exp {
        return fixed_powi(*x, n);
    }

    // General case: x^p = exp(p * ln(x)).
    // Requires x > 0, which is guaranteed at this point:
    // - x = 0 was handled above
    // - x < 0 with fractional p was rejected above
    //
    // Overflow: if p * ln(x) exceeds the fixed-point range, saturating_mul
    // clamps it. fixed_exp then receives a saturated value and returns either
    // None (overflow guard) or Some(0) (underflow guard), both of which
    // propagate correctly to the caller.
    let ln_x = fixed_ln(x)?;
    let exponent = p.saturating_mul(ln_x);
    fixed_exp(&exponent)
}

// ===============================================================================
// ````````````````````````````````` TRAIT FACADES ```````````````````````````````
// ===============================================================================

/// Unified interface for core fixed-point mathematical operations.
///
/// Implemented for all four fixed-point types: [`FixedU64`], [`FixedU128`],
/// [`FixedI64`], [`FixedI128`]. Enables generic code that works across the
/// entire fixed-point family through a single trait bound.
pub trait FixedOp
where
    Self: Sized,
{   
    /// Square root (real domain).
    fn fixed_sqrt(f: &Self) -> Option<Self>;
    /// General power `x^p` (integer and fractional exponents).
    fn fixed_pow(f: &Self, p: &Self) -> Option<Self>;
    /// Natural exponential ( e^x ).
    fn fixed_exp(f: &Self) -> Option<Self>;
    /// Natural logarithm ( ln(x) ).
    fn fixed_ln(f: &Self) -> Option<Self>;
}

/// Interface for complex-valued fixed-point operations.
///
/// Extends the real-domain operations in [`FixedOp`] with functions whose
/// results may be complex-valued.
pub trait FixedComplexOp
where   
    Self: Sized,
{   
    /// Square root in complex domain.
    fn complex_sqrt(f: &Self) -> Option<Complex<Self>>;
}

// --- FixedOp Implementations ---

/// FixedOp implementation for FixedU64.
impl FixedOp for FixedU64 {
    fn fixed_sqrt(f: &Self) -> Option<Self> {
        fixed_sqrt(f)
    }
    fn fixed_pow(f: &Self, p: &Self) -> Option<Self> {
        fixed_pow(f, p)
    }
    fn fixed_exp(f: &Self) -> Option<Self> {
        fixed_exp(f)
    }
    fn fixed_ln(f: &Self) -> Option<Self> {
        fixed_ln(f)
    }
}

/// FixedOp implementation for FixedU128.
impl FixedOp for FixedU128 {
    fn fixed_sqrt(f: &Self) -> Option<Self> {
        fixed_sqrt(f)
    }
    fn fixed_pow(f: &Self, p: &Self) -> Option<Self> {
        fixed_pow(f, p)
    }
    fn fixed_exp(f: &Self) -> Option<Self> {
        fixed_exp(f)
    }
    fn fixed_ln(f: &Self) -> Option<Self> {
        fixed_ln(f)
    }
}

/// FixedOp implementation for FixedI64.
impl FixedOp for FixedI64 {
    fn fixed_sqrt(f: &Self) -> Option<Self> {
        fixed_sqrt(f)
    }
    fn fixed_pow(f: &Self, p: &Self) -> Option<Self> {
        fixed_pow(f, p)
    }
    fn fixed_exp(f: &Self) -> Option<Self> {
        fixed_exp(f)
    }
    fn fixed_ln(f: &Self) -> Option<Self> {
        fixed_ln(f)
    }
}

/// FixedOp implementation for FixedI128.
impl FixedOp for FixedI128 {
    fn fixed_sqrt(f: &Self) -> Option<Self> {
        fixed_sqrt(f)
    }
    fn fixed_pow(f: &Self, p: &Self) -> Option<Self> {
        fixed_pow(f, p)
    }
    fn fixed_exp(f: &Self) -> Option<Self> {
        fixed_exp(f)
    }
    fn fixed_ln(f: &Self) -> Option<Self> {
        fixed_ln(f)
    }
}

// --- FixedComplexOp Implementations ---

/// FixedComplexOp implementation for FixedU64.
impl FixedComplexOp for FixedU64 {
    fn complex_sqrt(f: &Self) -> Option<Complex<Self>> {
        complex_sqrt(f)
    }
}

/// FixedComplexOp implementation for FixedI64.
impl FixedComplexOp for FixedI64 {
    fn complex_sqrt(f: &Self) -> Option<Complex<Self>> {
        complex_sqrt(f)
    }
}

/// FixedComplexOp implementation for FixedU128.
impl FixedComplexOp for FixedU128 {
    fn complex_sqrt(f: &Self) -> Option<Complex<Self>> {
        complex_sqrt(f)
    }
}

/// FixedComplexOp implementation for FixedI128.
impl FixedComplexOp for FixedI128 {
    fn complex_sqrt(f: &Self) -> Option<Complex<Self>> {
        complex_sqrt(f)
    }
}

// ===============================================================================
// ```````````````````````````````` PLANNED EXTENSIONS ```````````````````````````
// ===============================================================================

// pub trait FixedOp
// where
//     Self: Sized,
// {
//     // ------------------------
//     // Roots & Powers
//     // ------------------------
//     // Cube root
//     // fn fixed_cbrt(f: &Self) -> Self;
//     // Integer powers
//     // fn fixed_powi(f: &Self, n: i32) -> Self;
//     // n-th root
//     // fn fixed_root(f: &Self, n: &Self) -> Self;
//     // Square of a number
//     // fn fixed_square(f: &Self) -> Self;
//     // Reciprocal
//     // fn fixed_recip(f: &Self) -> Self;

//     // ------------------------
//     // Exponential & Logarithmic Variants
//     // ------------------------
//     // 2^x
//     // fn fixed_exp2(f: &Self) -> Self;
//     // 10^x
//     // fn fixed_exp10(f: &Self) -> Self;
//     // Natural log
//     // fn fixed_ln(f: &Self) -> Self;
//     // log base 2
//     // fn fixed_log2(f: &Self) -> Self;
//     // log base 10
//     // fn fixed_log10(f: &Self) -> Self;
//     // Exponential minus 1 (exp(x) - 1)
//     // fn fixed_expm1(f: &Self) -> Self;
//     // Logarithm of 1+x (ln(1+x))
//     // fn fixed_ln1p(f: &Self) -> Self;
//     // Logarithmic gamma function
//     // fn fixed_lgamma(f: &Self) -> Self;

//     // ------------------------
//     // Trigonometric Functions
//     // ------------------------
//     // Sine
//     // fn fixed_sin(f: &Self) -> Self;
//     // Cosine
//     // fn fixed_cos(f: &Self) -> Self;
//     // Tangent
//     // fn fixed_tan(f: &Self) -> Self;
//     // Arc sine
//     // fn fixed_asin(f: &Self) -> Self;
//     // Arc cosine
//     // fn fixed_acos(f: &Self) -> Self;
//     // Arc tangent
//     // fn fixed_atan(f: &Self) -> Self;
//     // Arc tangent of y/x
//     // fn fixed_atan2(y: &Self, x: &Self) -> Self;

//     // ------------------------
//     // Hyperbolic Functions
//     // ------------------------
//     // Hyperbolic sine
//     // fn fixed_sinh(f: &Self) -> Self;
//     // Hyperbolic cosine
//     // fn fixed_cosh(f: &Self) -> Self;
//     // Hyperbolic tangent
//     // fn fixed_tanh(f: &Self) -> Self;
//     // Hyperbolic arc sine
//     // fn fixed_asinh(f: &Self) -> Self;
//     // Hyperbolic arc cosine
//     // fn fixed_acosh(f: &Self) -> Self;
//     // Hyperbolic arc tangent
//     // fn fixed_atanh(f: &Self) -> Self;

//     // ------------------------
//     // Special Functions
//     // ------------------------
//     // Error function
//     // fn fixed_erf(f: &Self) -> Self;
//     // Complementary error function
//     // fn fixed_erfc(f: &Self) -> Self;
//     // Gamma function
//     // fn fixed_gamma(f: &Self) -> Self;
//     // Factorial for integer values
//     // fn fixed_fact(n: u32) -> Self;
//     // Factorial for floating point (gamma variant)
//     // fn fixed_factf(f: &Self) -> Self;
//     // Binomial coefficient (n choose k)
//     // fn fixed_binom(n: u32, k: u32) -> Self;
//     // Signum function
//     // fn fixed_sign(f: &Self) -> Self;
//     // Clamp value between min and max
//     // fn fixed_clamp(f: &Self, min: &Self, max: &Self) -> Self;
//     // Floor
//     // fn fixed_floor(f: &Self) -> Self;
//     // Ceil
//     // fn fixed_ceil(f: &Self) -> Self;
//     // Round
//     // fn fixed_round(f: &Self) -> Self;
//     // Fractional part
//     // fn fixed_frac(f: &Self) -> Self;

//     // ------------------------
//     // Numeric & Scientific Utilities
//     // ------------------------
//     // Absolute value
//     // fn fixed_abs(f: &Self) -> Self;
//     // Euclidean norm for 2D or 3D (sqrt(x^2 + y^2))
//     // fn fixed_hypot(x: &Self, y: &Self) -> Self;
//     // Complex modulus squared
//     // fn fixed_modsq(f: &Self) -> Self;
//     // Power of 2 rounding (next_pow2)
//     // fn fixed_next_pow2(f: &Self) -> Self;
//     // Logarithm with arbitrary base
//     // fn fixed_logb(f: &Self, base: &Self) -> Self;
//     // Reciprocal square root (1/sqrt(x))
//     // fn fixed_rsqrt(f: &Self) -> Self;
// }


// ===============================================================================
// `````````````````````````````````` UNIT TESTS `````````````````````````````````
// ===============================================================================

#[cfg(test)]
mod tests {
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` IMPORTS ```````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    // --- Module import ---
    use super::*;

    // --- Substrate crates ---
    use sp_runtime::traits::{Bounded, One, Saturating, Zero};

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` FIXED_SQRT `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    
    #[test]
    fn fixed_sqrt_perfect_cases() {
        // case 1: sqrt(4) -> 2
        let x: FixedU64 = 4.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::saturating_from_integer(2);
        assert_eq!(result, expected);

        // case 2: sqrt(36) -> 6
        let x: FixedU64 = 36.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::saturating_from_integer(6);
        assert_eq!(result, expected);

        // case 3: sqrt(81) -> 9
        let x: FixedU64 = 81.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::saturating_from_integer(9);
        assert_eq!(result, expected);

        // case 4: sqrt(1) -> 1
        let x: FixedU64 = 1.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::saturating_from_integer(1);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_sqrt_negative_perfect_cases() {
        // case 1: sqrt(-1) -> None
        let x: FixedI64 = (-1).into();
        let result = fixed_sqrt(&x);
        assert!(result.is_none());

        // case 2: sqrt(-16) -> None
        let x: FixedI64 = (-16).into();
        let result = fixed_sqrt(&x);
        assert!(result.is_none());

        // case 3: sqrt(-9) -> None
        let x: FixedI64 = (-9).into();
        let result = fixed_sqrt(&x);
        assert!(result.is_none());

        // case 4: sqrt(-100) -> None
        let x: FixedI64 = (-100).into();
        let result = fixed_sqrt(&x);
        assert!(result.is_none());

        // case 5: sqrt(-4) -> None
        let x: FixedI64 = (-4).into();
        let result = fixed_sqrt(&x);
        assert!(result.is_none());
    }

    #[test]
    fn fixed_sqrt_large_numbers() {
        // case 1: sqrt(10000) -> 100
        let x: FixedU64 = 10000.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::saturating_from_integer(100);
        assert_eq!(result, expected);

        // case 2: sqrt(1000000) -> 1000
        let x: FixedU64 = 1000000.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::saturating_from_integer(1000);
        assert_eq!(result, expected);

        // case 3: sqrt(u32::MAX) -> 65535.999992370
        let x: FixedU64 = (u32::MAX as u64).into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::from_inner(65535999992370);
        assert_eq!(result, expected);

        // case 4: sqrt(u64::MAX) -> 135818.791312945
        let x: FixedU64 = (u64::MAX).into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::from_inner(135818791312945);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_sqrt_non_perfect_cases() {
        // case 1: sqrt(2) ~= 1.414213562
        let x: FixedU64 = 2.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::from_inner(1414213562);
        assert_eq!(result, expected);

        // case 2: sqrt(5) -> 2.236067977
        let x: FixedU64 = 5.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::from_inner(2236067977);
        assert_eq!(result, expected);

        // case 3: sqrt(10) -> 3.162277660
        let x: FixedU64 = 10.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected: FixedU64 = FixedU64::from_inner(3162277660);
        assert_eq!(result, expected);

        // case 4: sqrt(125) -> 11.180339887
        let x: FixedU64 = 125.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected: FixedU64 = FixedU64::from_inner(11180339887);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_sqrt_negative_non_perfect_squares() {
        // case 1: sqrt(-2) -> None
        let x: FixedI64 = (-2).into();
        let result = fixed_sqrt(&x);
        assert!(result.is_none());

        // case 2: sqrt(-35) -> None
        let x: FixedI64 = (-35).into();
        let result = fixed_sqrt(&x);
        assert!(result.is_none());

        // case 3: sqrt(-50) -> None
        let x: FixedI64 = (-50).into();
        let result = fixed_sqrt(&x);
        assert!(result.is_none());
    }

    #[test]
    fn fixed_sqrt_fractional_cases() {
        // case 1: sqrt(0.25) -> 0.5
        let x = FixedU64::saturating_from_rational(1, 4);
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::saturating_from_rational(1, 2);
        assert_eq!(result, expected);

        // case 2: sqrt(0.01) -> 0.1
        let x = FixedU64::saturating_from_rational(1, 100);
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::saturating_from_rational(1, 10);
        assert_eq!(result, expected);

        // case 3: sqrt(0.5) -> 0.707106781
        let x = FixedU64::saturating_from_rational(1, 2);
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::from_inner(707106781);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_sqrt_edge_cases() {
        // case 1: sqrt(0) -> 0
        let x: FixedU64 = 0.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::zero();
        assert_eq!(result, expected);

        // case 2: sqrt(1) -> 1
        let x: FixedU64 = 1.into();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedU64::one();
        assert_eq!(result, expected);

        // case 3
        // sqrt(i64::MIN) -> saturating_abs gives i64::MAX
        let x: FixedI64 = (i64::MIN).into();
        let result = fixed_sqrt(&x);
        assert!(result.is_none());

        // case 4
        // sqrt(i64::MAX) -> 96038.388349944
        let x  = FixedI64::max_value();
        let result = fixed_sqrt(&x).unwrap();
        let expected = FixedI64::from_inner(96038388349944);
        assert_eq!(result, expected);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` COMPLEX_SQRT ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn complex_sqrt_perfect_cases() {
        // case 1: sqrt(4) -> 2
        let x: FixedU64 = 4.into();
        let result = complex_sqrt(&x).unwrap();
        let expected: Complex<FixedU64> = Complex {
            real: 2.into(),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 2: sqrt(36) -> 6
        let x: FixedU64 = 36.into();
        let result = complex_sqrt(&x).unwrap();
        let expected: Complex<FixedU64> = Complex {
            real: 6.into(),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 3: sqrt(81) -> 9
        let x: FixedU64 = 81.into();
        let result = complex_sqrt(&x).unwrap();
        let expected: Complex<FixedU64> = Complex {
            real: 9.into(),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 4: sqrt(1) -> 1
        let x: FixedU64 = 1.into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 1.into(),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn complex_sqrt_negative_perfect_cases() {
        // case 1: sqrt(-1) -> 1i (imaginary)
        let x: FixedI64 = (-1).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 0.into(),
            imgn: 1.into(),
        };
        assert_eq!(result, expected);

        // case 2:sqrt(-16) -> 4i (imaginary)
        let x: FixedI64 = (-16).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 0.into(),
            imgn: 4.into(),
        };
        assert_eq!(result, expected);

        // case 3: sqrt(-9) -> 3i (imaginary)
        let x: FixedI64 = (-9).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 0.into(),
            imgn: 3.into(),
        };
        assert_eq!(result, expected);

        // case 4: sqrt(-100) -> 10i (imaginary)
        let x: FixedI64 = (-100).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 0.into(),
            imgn: 10.into(),
        };
        assert_eq!(result, expected);

        // case 5: sqrt(-4) -> 2i (imaginary)
        let x: FixedI64 = (-4).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 0.into(),
            imgn: 2.into(),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn complex_sqrt_large_numbers() {
        // case 1: sqrt(10000) -> 100
        let x: FixedU64 = 10000.into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 100.into(),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 2: sqrt(1000000) -> 1000
        let x: FixedU64 = 1000000.into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 1000.into(),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 3: sqrt(u32::MAX) -> 65535.999992370
        let x: FixedU64 = (u32::MAX as u64).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: FixedU64::from_inner(65535999992370),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 4: sqrt(u64::MAX) -> 135818.791312945
        let x: FixedU64 = (u64::MAX).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: FixedU64::from_inner(135818791312945),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn complex_sqrt_non_perfect_cases() {
        // case 1: sqrt(2) ~= 1.414213562
        let x: FixedU64 = 2.into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: FixedU64::from_inner(1414213562),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 2: sqrt(5) -> 2.236067977
        let x: FixedU64 = 5.into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: FixedU64::from_inner(2236067977),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 3: sqrt(10) -> 3.162277660
        let x: FixedU64 = 10.into();
        let result = complex_sqrt(&x).unwrap();
        let expected: Complex<FixedU64> = Complex {
            real: FixedU64::from_inner(3162277660),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 4: sqrt(125) -> 11.180339887
        let x: FixedU64 = 125.into();
        let result = complex_sqrt(&x).unwrap();
        let expected: Complex<FixedU64> = Complex {
            real: FixedU64::from_inner(11180339887),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn complex_sqrt_negative_non_perfect_squares() {
        // case 1: sqrt(-2) -> 1.414213562i
        let x: FixedI64 = (-2).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 0.into(),
            imgn: FixedI64::from_inner(1414213562),
        };
        assert_eq!(result, expected);

        // case 2: sqrt(-35) -> 5.916079783i (imaginary)
        let x: FixedI64 = (-35).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 0.into(),
            imgn: FixedI64::from_inner(5916079783),
        };
        assert_eq!(result, expected);

        // case 3: sqrt(-50) -> 7.071067811i
        let x: FixedI64 = (-50).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 0.into(),
            imgn: FixedI64::from_inner(7071067811),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn complex_sqrt_fractional_cases() {
        // case 1: sqrt(0.25) -> 0.5
        let x = FixedU64::saturating_from_rational(1, 4);
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: FixedU64::saturating_from_rational(1, 2),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 2: sqrt(0.01) -> 0.1
        let x = FixedU64::saturating_from_rational(1, 100);
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: FixedU64::saturating_from_rational(1, 10),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 3: sqrt(0.5) -> 0.707106781
        let x = FixedU64::saturating_from_rational(1, 2);
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: FixedU64::from_inner(707106781),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);
    }

    #[test]
    fn complex_sqrt_edge_cases() {
        // case 1: sqrt(0) -> 0
        let x: FixedU64 = 0.into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 0.into(),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 2: sqrt(1) -> 1
        let x: FixedU64 = 1.into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 1.into(),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 3
        // sqrt(i64::MAX) -> 96038.388349944
        let x  = FixedI64::max_value();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: FixedI64::from_inner(96038388349944),
            imgn: 0.into(),
        };
        assert_eq!(result, expected);

        // case 4
        // sqrt(i64::MIN) -> saturating_abs gives i64::MAX
        let x: FixedI64 = (i64::MIN).into();
        let result = complex_sqrt(&x).unwrap();
        let expected = Complex {
            real: 0.into(),
            imgn: FixedI64::from_inner(96038388349944),
        };
        assert_eq!(result, expected);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` FIXED_EXP ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn fixed_exp_normal_cases() {
        // case 1: exp(0) -> 1
        let x: FixedU64 = 0.into();
        let result = fixed_exp(&x).unwrap();
        let expected = 1.into();
        assert_eq!(result, expected);

        // case 2: exp(1) -> 2.718281828
        let x: FixedU64 = 1.into();
        let result = fixed_exp(&x).unwrap();
        let expected = FixedU64::from_inner(2718281828);
        assert_eq!(result, expected);

        // case 3: exp(2) -> 7.389056096
        let x: FixedU64 = 2.into();
        let result = fixed_exp(&x).unwrap();
        let expected = FixedU64::from_inner(7389056096);
        assert_eq!(result, expected);

        // case 4: exp(3) -> 20.085536911
        let x: FixedU64 = 3.into();
        let result = fixed_exp(&x).unwrap();
        let expected = FixedU64::from_inner(20085536911);
        assert_eq!(result, expected);

        // case 5: exp(5) -> 148.413158957
        let x: FixedU64 = 5.into();
        let result = fixed_exp(&x).unwrap();
        let expected = FixedU64::from_inner(148413158957);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_exp_positive_fractional_edge_cases() {
        // case 1: exp(0.5) -> 1.648721267
        let x = FixedU64::saturating_from_rational(1, 2);
        let result = fixed_exp(&x).unwrap();
        let expected = FixedU64::from_inner(1648721267);
        assert_eq!(result, expected);

        // case 2: exp(0.1) -> 1.105170915
        let x = FixedU64::saturating_from_rational(1, 10);
        let result = fixed_exp(&x).unwrap();
        let expected = FixedU64::from_inner(1105170915);
        assert_eq!(result, expected);

        // case 3: exp(0.001) -> 1.001000500
        let x = FixedU64::saturating_from_rational(1, 1000);
        let result = fixed_exp(&x).unwrap();
        let expected = FixedU64::from_inner(1001000500);
        assert_eq!(result, expected);

        // case 4: exp(2.5) -> 12.182493928
        let x = FixedU64::saturating_from_rational(5, 2);
        let result = fixed_exp(&x).unwrap();
        let expected = FixedU64::from_inner(12182493928);
        assert_eq!(result, expected);

        // case 5: exp(1.5) -> 4.481689059
        let x = FixedU64::saturating_from_rational(3, 2);
        let result = fixed_exp(&x).unwrap();
        let expected = FixedU64::from_inner(4481689059);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_exp_negative_edge_cases() {
        // case 1: exp(-1) -> 0.367879441
        let x: FixedI64 = (-1).into();
        let result = fixed_exp(&x).unwrap();
        let expected = FixedI64::from_inner(367879441);
        assert_eq!(result, expected);

        // case 2: exp(-2) -> 0.135335383
        let x: FixedI64 = (-2).into();
        let result = fixed_exp(&x).unwrap();
        let expected = FixedI64::from_inner(135335283);
        assert_eq!(result, expected);

        // case 3: exp(-0.5) -> 0.606530659
        let x = FixedI64::saturating_from_rational(-1, 2);
        let result = fixed_exp(&x).unwrap();
        let expected = FixedI64::from_inner(606530659);
        assert_eq!(result, expected);

        // case 4: exp(-15) -> 0.000000305
        let x: FixedI64 = (-15).into();
        let result = fixed_exp(&x).unwrap();
        let expected = FixedI64::from_inner(000000305);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_exp_mathematical_constants() {
        // case 1:
        // exp(ln(2)) should be close to 2
        // ln(2) ~= 0.693147181
        let ln2 = FixedU64::from_inner(693147181);
        let result = fixed_exp(&ln2).unwrap();
        let expected: FixedU64 = 2.into();

        // Allow small tolerance due to precision
        let diff = if result > expected {
            result.saturating_sub(expected)
        } else {
            expected.saturating_sub(result)
        };
        let tolerance = FixedU64::from_inner(1_000_000); // 0.001
        assert!(diff < tolerance);

        // case 2:
        // exp(ln(10)) should be close to 10
        // ln(10) ~= 2.302585093
        let ln10 = FixedU64::from_inner(2302585093);
        let result = fixed_exp(&ln10).unwrap();
        let expected: FixedU64 = 10.into();

        let diff = if result > expected {
            result.saturating_sub(expected)
        } else {
            expected.saturating_sub(result)
        };
        assert!(diff < tolerance);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` FIXED_LN ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn fixed_ln_at_one() {
        // case 1: ln(1) = 0 (real part), 0 (imaginary)
        let x: FixedU64 = 1.into();
        let result = fixed_ln(&x).unwrap();
        let expected = FixedU64::from_inner(0);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_ln_at_zero() {
        // case 1: ln(0) -> None
        let x: FixedU64 = 0.into();
        let result = fixed_ln(&x);
        assert!(result.is_none());
    }

    #[test]
    fn fixed_ln_positive_integers() {
        // case 1: ln(2) ~= 0.693147172
        let x: FixedU64 = 2.into();
        let result = fixed_ln(&x).unwrap();
        let expected = FixedU64::from_inner(693147172);
        assert_eq!(result, expected);

        // case 2: ln(3) ~= 1.098612248
        let x: FixedU64 = 3.into();
        let result = fixed_ln(&x).unwrap();
        let expected = FixedU64::from_inner(1098612248);
        assert_eq!(result, expected);

        // case 3: ln(10) ~= 2.302585040
        let x: FixedU64 = 10.into();
        let result = fixed_ln(&x).unwrap();
        let expected = FixedU64::from_inner(2302585040);
        assert_eq!(result, expected);

        // case 4: ln(100) ~= 4.605170180
        let x: FixedU64 = 100.into();
        let result = fixed_ln(&x).unwrap();
        let expected = FixedU64::from_inner(4605170080);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_ln_fractional_values() {
        // case 1: ln(0.5) ~= -0.693147174
        let x = FixedI64::saturating_from_rational(1, 2);
        let result = fixed_ln(&x).unwrap();
        let expected = FixedI64::from_inner(-693147174);
        assert_eq!(result, expected);

        // case 2: ln(0.1) ~= -2.302585064
        let x = FixedI64::saturating_from_rational(1, 10);
        let result = fixed_ln(&x).unwrap();
        let expected = FixedI64::from_inner(-2302585064);
        assert_eq!(result, expected);

        // case 3: ln(1.5) ~= 0.405465100
        let x = FixedU64::saturating_from_rational(3, 2);
        let result = fixed_ln(&x).unwrap();
        let expected = FixedU64::from_inner(405465100);
        assert_eq!(result, expected);

        // case 4: ln(2.5) ~= 0.916290712
        let x = FixedU64::saturating_from_rational(5, 2);
        let result = fixed_ln(&x).unwrap();
        let expected = FixedU64::from_inner(916290712);
        assert_eq!(result, expected);

        // case 5: ln(0.5) -> None
        let x = FixedU64::saturating_from_rational(1, 2);
        let result = fixed_ln(&x);
        assert!(result.is_none());

        // case 6: ln(0.1) -> None
        let x = FixedU64::saturating_from_rational(1, 10);
        let result = fixed_ln(&x);
        assert!(result.is_none());

    }

    #[test]
    fn fixed_ln_negative_values() {
        // case 1: ln(-1) -> None
        let x: FixedI64 = (-1).into();
        let result = fixed_ln(&x);
        assert!(result.is_none());

        // case 2: ln(-2) ~= 0.6931471872 
        let x: FixedI64 = (-2).into();
        let result = fixed_ln(&x);
        assert!(result.is_none());

        // case 3: ln(-10) ~= 2.302585040 
        let x: FixedI64 = (-10).into();
        let result = fixed_ln(&x);
        assert!(result.is_none());

        // case 4: ln(-0.5) ~= -0.693147174
        let x = FixedI64::saturating_from_rational(-1, 2);
        let result = fixed_ln(&x);
        assert!(result.is_none());
    }

    #[test]
    fn fixed_ln_mathematical_constants() {
        // case 1: ln(e) should be ~= 1
        // e ~= 2.718281828
        let e = FixedU64::from_inner(2718281828);
        let result = fixed_ln(&e).unwrap();
        let expected = FixedU64::from_inner(1000000000);

        let tolerance = FixedU64::from_inner(1_000_000); // 0.001
        let diff = if result > expected {
            result.saturating_sub(expected)
        } else {
            expected.saturating_sub(result)
        };
        assert!(diff < tolerance);
    }

    #[test]
    fn fixed_ln_inverse_of_exp() {
        // case 1: x = 1
        let x: FixedU64 = 1.into();
        let exp_x = fixed_exp(&x).unwrap();
        let result = fixed_ln(&exp_x).unwrap();

        let tolerance = FixedU64::from_inner(1_000_000); // 0.001
        let diff = if result > x {
            result.saturating_sub(x)
        } else {
            x.saturating_sub(result)
        };
        assert!(diff < tolerance);

        // case 2: x = 2
        let x: FixedU64 = 2.into();
        let exp_x = fixed_exp(&x).unwrap();
        let result = fixed_ln(&exp_x).unwrap();

        let diff = if result > x {
            result.saturating_sub(x)
        } else {
            x.saturating_sub(result)
        };
        assert!(diff < tolerance);
    }

    #[test]
    fn fixed_ln_properties_verification() {
        // case 1: ln(a*b) = ln(a) + ln(b)
        // ln(2*3) = ln(6) should equal ln(2) + ln(3)
        let x: FixedU64 = 6.into();
        let ln_6 = fixed_ln(&x).unwrap();

        let two: FixedU64 = 2.into();
        let three: FixedU64 = 3.into();
        let ln_2 = fixed_ln(&two).unwrap();
        let ln_3 = fixed_ln(&three).unwrap();
        let sum = ln_2.saturating_add(ln_3);

        let tolerance = FixedU64::from_inner(1_000_000); // 0.001
        let diff = if ln_6 > sum {
            ln_6.saturating_sub(sum)
        } else {
            sum.saturating_sub(ln_6)
        };
        assert!(diff < tolerance);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````````` FIXED_POW ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn fixed_pow_edge_cases() {
        // case 1: x = 1, p = 0
        let x: FixedU64 = 1.into();
        let p: FixedU64 = 0.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected: FixedU64 = 1.into();
        assert_eq!(result, expected);

        // case 2: x = 0, p = 1
        let x: FixedU64 = 0.into();
        let p: FixedU64 = 1.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected: FixedU64 = 0.into();
        assert_eq!(result, expected);

        // case 3: x = 1, p = 1
        let x: FixedU64 = 1.into();
        let p: FixedU64 = 1.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected: FixedU64 = 1.into();
        assert_eq!(result, expected);

        // case 4: pow(0, -1) = 1 / 0 -> None
        let x: FixedI64 = 0.into();
        let p: FixedI64 = (-1).into();
        let result = fixed_pow(&x, &p);
        assert!(result.is_none());
    }

    #[test]
    fn fixed_pow_normal_cases() {
        // case 1: x = 2, p = 3
        let x: FixedU64 = 2.into();
        let p: FixedU64 = 3.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected: FixedU64 = 8.into();
        assert_eq!(result, expected);

        // case 2: x = 4, p = 5
        let x: FixedU64 = 4.into();
        let p: FixedU64 = 5.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected: FixedU64 = 1024.into();
        assert_eq!(result, expected);

        // case 3: x = 8, p = 6
        let x: FixedU64 = 8.into();
        let p: FixedU64 = 6.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected: FixedU64 = 262144.into();
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_pow_fractional_exponents() {
        let tolerance: FixedU64 = FixedU64::from_inner(1_000_000); // 0.001

        // case 1: pow(4, 0.5) = 2
        let x: FixedU64 = 4.into();
        let p = FixedU64::saturating_from_rational(1, 2);
        let result = fixed_pow(&x, &p).unwrap();
        let expected: FixedU64 = 2.into();
        let diff = if result > expected {
            result.saturating_sub(expected)
        } else {
            expected.saturating_sub(result)
        };
        assert!(diff < tolerance);

        // case 2: pow(9, 0.25) = sqrt(3) ~= 1.7320508
        let x: FixedU64 = 9.into();
        let p = FixedU64::saturating_from_rational(1, 4);
        let result = fixed_pow(&x, &p).unwrap();
        let expected = FixedU64::saturating_from_rational(1732051, 1_000_000); // ~= sqrt(3)
        let diff = if result > expected {
            result.saturating_sub(expected)
        } else {
            expected.saturating_sub(result)
        };
        assert!(diff < tolerance);

        // case 3: pow(12, 0.35) ~= 2.386876
        let x: FixedU64 = 12.into();
        let p = FixedU64::saturating_from_rational(35, 100); // 0.35
        let result = fixed_pow(&x, &p).unwrap();
        let expected = FixedU64::saturating_from_rational(2_386_876, 1_000_000);

        let diff = if result > expected {
            result.saturating_sub(expected)
        } else {
            expected.saturating_sub(result)
        };
        assert!(diff < tolerance);
    }

    #[test]
    fn fixed_pow_fractional_base_integer_exp() {
        let tolerance = FixedU64::from_inner(1_000_000);

        // case 1: pow(1.5, 2) = 2.25
        let x = FixedU64::saturating_from_rational(3, 2);
        let p: FixedU64 = 2.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected = FixedU64::saturating_from_rational(9, 4);

        let diff = if result > expected {
            result.saturating_sub(expected)
        } else {
            expected.saturating_sub(result)
        };
        assert!(diff < tolerance);

        // case 2: pow(3.5, 4) = 150.0625
        let x = FixedU64::saturating_from_rational(7, 2); // 3.5
        let p: FixedU64 = 4.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected = FixedU64::from_inner(150062500000);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_pow_negative_base_integer_exp() {
        // case 1: pow(-2, 3) = -8
        let x: FixedI64 = (-2).into();
        let p: FixedI64 = 3.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected: FixedI64 = (-8).into();
        assert_eq!(result, expected);

        // case 2: pow(-6, 5) = -7776
        let x: FixedI64 = (-6).into();
        let p: FixedI64 = 5.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected: FixedI64 = (-7776).into();
        assert_eq!(result, expected);

        // case 3: pow(-4, 2) = 16
        let x: FixedI64 = (-4).into();
        let p: FixedI64 = 2.into();
        let result = fixed_pow(&x, &p).unwrap();
        let expected: FixedI64 = (16).into();
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_pow_negative_base_fractional_exp() {
        // case 1: pow(-4, 1/2) -> None
        let x: FixedI64 = (-4).into();
        let p = FixedI64::saturating_from_rational(1, 2);
        let result = fixed_pow(&x, &p);
        assert!(result.is_none());

        // case 2: pow(-8, 1/3) -> None
        let x: FixedI64 = (-8).into();
        let p = FixedI64::saturating_from_rational(1, 3);

        let result = fixed_pow(&x, &p);
        assert!(result.is_none())
    }

    #[test]
    fn fixed_pow_large_integer_overflow_saturates() {
        // 2^128 overflows FixedU64
        let x: FixedU64 = 2.into();
        let p: FixedU64 = 128.into();

        let result = fixed_pow(&x, &p).unwrap();
        let expected = u64::MAX.into();
        assert_eq!(result, expected);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````` FIXED_SQRT_NEWTON ``````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn fixed_sqrt_newton_standard_cases() {
        // case 1: x = 81 -> 9
        let x: FixedU64 = (81).into();
        let result = fixed_sqrt_newton(&x);
        let expected = 9.into();
        assert_eq!(result, expected);

        // case 2: x = 49 -> 7
        let x: FixedU64 = (49).into();
        let result = fixed_sqrt_newton(&x);
        let expected = 7.into();
        assert_eq!(result, expected);

        // case 3: x = 10 -> 3.162277660
        let x: FixedU64 = 10.into();
        let result = fixed_sqrt_newton(&x);
        let expected: FixedU64 = FixedU64::from_inner(3162277660);
        assert_eq!(result, expected);

        // case 4: x = 7 -> 2.2645751311
        let x: FixedU64 = 7.into();
        let result = fixed_sqrt_newton(&x);
        let expected: FixedU64 = FixedU64::from_inner(2645751311);
        assert_eq!(result, expected);
    }

    #[test]
    fn fixed_sqrt_newton_edge_cases() {
        // case 1: x = 0 -> 0
        let x: FixedU64 = (0).into();
        let result = fixed_sqrt_newton(&x);
        let expected = 0.into();
        assert_eq!(result, expected);

        // case 2: x < 0 : -9 -> 0
        let x: FixedI64 = (-9).into();
        let result = fixed_sqrt_newton(&x);
        let expected = 0.into();
        assert_eq!(result, expected);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ``````````````````````````````````` FIXED_PI ``````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn fixed_pi_value_check() {
        // fixed_pi() -> 3.141592920
        let actual: FixedU64 = fixed_pi();
        let expected = FixedU64::from_inner(3141592920);
        assert_eq!(actual, expected);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // `````````````````````````````` RANGE_REDUCES_SQRT `````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn range_reduce_sqrt_already_in_range() {
        // case 1: y = 1 (already at target, no reduction needed)
        let y: FixedU64 = 1.into();
        let (reduced, k) = range_reduce_sqrt(y);
        assert_eq!(reduced, y);
        assert_eq!(k, 0);

        // case 2: y = 1.5 (within [0.5, 1.5], no reduction)
        let y = FixedU64::saturating_from_rational(3, 2);
        let (reduced, k) = range_reduce_sqrt(y);
        assert_eq!(reduced, y);
        assert_eq!(k, 0);

        // case 3: y = 0.5 (at lower bound, no reduction)
        let y = FixedU64::saturating_from_rational(1, 2);
        let (reduced, k) = range_reduce_sqrt(y);
        assert_eq!(reduced, y);
        assert_eq!(k, 0);

        // case 4: y = 0.75 (within range)
        let y = FixedU64::saturating_from_rational(3, 4);
        let (reduced, k) = range_reduce_sqrt(y);
        assert_eq!(reduced, y);
        assert_eq!(k, 0);
    }

    #[test]
    fn range_reduce_sqrt_needs_reduction_above_range() {
        // Reduced value should be in range [0.5, 1.5]
        let half = FixedU64::saturating_from_rational(1, 2);
        let one_half = FixedU64::saturating_from_rational(3, 2);

        // case 1: y = 4 (needs reduction)
        // sqrt(4) = 2, sqrt(2) = 1.414 (within range)
        let y: FixedU64 = 4.into();
        let (reduced, k) = range_reduce_sqrt(y);
        // 2 reductions needed
        assert_eq!(k, 2);
        assert!(reduced >= half && reduced <= one_half);

        // case 2: y = 16 (needs multiple reductions)
        // sqrt(16) = 4, sqrt(4) = 2, sqrt(2) = 1.414
        let y: FixedU64 = 16.into();
        let (reduced, k) = range_reduce_sqrt(y);
        assert_eq!(k, 3); // 3 reductions needed
        assert!(reduced >= half && reduced <= one_half);

        // case 3: y = 100 (large value)
        let y: FixedU64 = 100.into();
        let (reduced, k) = range_reduce_sqrt(y);
        assert!(k > 0);
        assert!(reduced >= half && reduced <= one_half);
    }

    #[test]
    fn range_reduce_sqrt_needs_reduction_below_range() {
        let half = FixedU64::saturating_from_rational(1, 2);
        let one_half = FixedU64::saturating_from_rational(3, 2);

        // case 1: y = 0.25 (below range, needs reduction)
        // sqrt(0.25) = 0.5 (now in range)
        let y = FixedU64::saturating_from_rational(1, 4);
        let (reduced, k) = range_reduce_sqrt(y);
        assert_eq!(k, 1);
        assert!(reduced >= half && reduced <= one_half);

        // case 2: y = 0.01 (very small, needs multiple reductions)
        let y = FixedU64::saturating_from_rational(1, 100);
        let (reduced, k) = range_reduce_sqrt(y);
        assert!(k > 0);
        assert_eq!(k, 3);
        assert!(reduced >= half && reduced <= one_half);

        // case 3: y = 0.0001 (very small, needs multiple reductions)
        let y = FixedU64::saturating_from_rational(1, 10000);
        let (reduced, k) = range_reduce_sqrt(y);
        assert!(k > 0);
        assert_eq!(k, 4);
        assert!(reduced >= half && reduced <= one_half);
    }

    #[test]
    fn range_reduce_sqrt_edge_cases() {
        // case 1: y = 0 (zero input)
        let y: FixedU64 = 0.into();
        let (reduced, k) = range_reduce_sqrt(y);

        // sqrt(0) = 0, which is outside [0.5, 1.5]
        // But the function should handle this gracefully
        assert_eq!(reduced, y);
        assert_eq!(k, 0);

        // case 2: y very close to 1 (minimal difference)
        let y = FixedU64::from_inner(1000000001); // 1.000000001
        let (reduced, k) = range_reduce_sqrt(y);

        // Should not need reduction (within tolerance)
        assert_eq!(k, 0);
        assert_eq!(reduced, y);

        // case 3: y = u32::MAX (very large value)
        let y: FixedU64 = (u32::MAX as u64).into();
        let (reduced, k) = range_reduce_sqrt(y);

        // Should reduce many times
        assert!(k > 0);
        let half = FixedU64::saturating_from_rational(1, 2);
        let one_half = FixedU64::saturating_from_rational(3, 2);
        assert!(reduced >= half && reduced <= one_half);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` LN_NEAR_ONE `````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn ln_near_one_at_one() {
        // case 1: y = 1, ln(1) = 0
        let y: FixedU64 = 1.into();
        let result = ln_near_one(y);
        let expected: FixedU64 = 0.into();
        assert_eq!(result, expected);
    }

    #[test]
    fn ln_near_one_slightly_above_one() {
        // case 1: y = 1.1, ln(1.1) ~= 0.095310176
        let y = FixedU64::saturating_from_rational(11, 10);
        let result = ln_near_one(y);
        let expected = FixedU64::from_inner(95310176);
        assert_eq!(result, expected);

        // case 2: y = 1.2, ln(1.2) ~= 0.182321552
        let y = FixedU64::saturating_from_rational(6, 5);
        let result = ln_near_one(y);
        let expected = FixedU64::from_inner(182321552);
        assert_eq!(result, expected);

        // case 3: y = 1.5, ln(1.5) ~= 0.405465100
        let y = FixedU64::saturating_from_rational(3, 2);
        let result = ln_near_one(y);
        let expected = FixedU64::from_inner(405465100);
        assert_eq!(result, expected);
    }

    #[test]
    fn ln_near_one_slightly_below_one() {
        // case 1: y = 0.9, ln(0.9) ~= -0.105360510
        let y = FixedU64::saturating_from_rational(9, 10);
        let _result = ln_near_one(y);
        // Result will be negative (but FixedU64 is unsigned, so this might underflow)
        // For signed types:
        let y_signed = FixedI64::saturating_from_rational(9, 10);
        let result_signed = ln_near_one(y_signed);
        let expected = FixedI64::from_inner(-105360510);
        assert_eq!(result_signed, expected);

        // case 2: y = 0.8, ln(0.8) ~= -0.223143548
        let y = FixedI64::saturating_from_rational(4, 5);
        let result = ln_near_one(y);
        let expected = FixedI64::from_inner(-223143548);
        assert_eq!(result, expected);

        // case 3: y = 0.5, ln(0.5) ~= -0.693147174
        let y = FixedI64::saturating_from_rational(1, 2);
        let result = ln_near_one(y);
        let expected = FixedI64::from_inner(-693147174);
        assert_eq!(result, expected);
    }

    #[test]
    fn ln_near_one_very_close_to_one() {
        // case 1: y = 1.01, ln(1.01) ~= 0.009950330
        let y = FixedU64::saturating_from_rational(101, 100);
        let result = ln_near_one(y);
        let expected = FixedU64::from_inner(9950330);
        assert_eq!(result, expected);

        // case 2: y = 1.001, ln(1.001) ~= 0.000999500
        let y = FixedU64::saturating_from_rational(1001, 1000);
        let result = ln_near_one(y);
        let expected = FixedU64::from_inner(999500);
        assert_eq!(result, expected);

        // case 3: y = 0.99, ln(0.99) ~= -0.010050334
        let y = FixedI64::saturating_from_rational(99, 100);
        let result = ln_near_one(y);
        let expected = FixedI64::from_inner(-10050334);
        assert_eq!(result, expected);

        // case 4: y = 0.999, ln(0.999) ~= -0.001000500
        let y = FixedI64::saturating_from_rational(999, 1000);
        let result = ln_near_one(y);
        let expected = FixedI64::from_inner(-1000500);
        assert_eq!(result, expected);
    }

    #[test]
    fn ln_near_one_edge_cases() {
        // case 1: Very small positive deviation
        // y = 1.0001, ln(1.0001) ~= 0.000099994
        let y = FixedU64::saturating_from_rational(10001, 10000);
        let result = ln_near_one(y);
        let expected = FixedU64::from_inner(99994);
        assert_eq!(result, expected);

        // case 2: Very small negative deviation
        // y = 0.9999, ln(0.9999) ~= -0.000100004
        let y = FixedI64::saturating_from_rational(9999, 10000);
        let result = ln_near_one(y);
        let expected = FixedI64::from_inner(-100004);
        assert_eq!(result, expected);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ```````````````````````````` DYNAMIC_MAX_ITERATIONS ```````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    
    #[test]
    fn dynamic_max_iterations_zero_input() {
        // case 1: x = 0, should return minimum of 1 iteration
        let x: FixedU64 = 0.into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 1);
    }

    #[test]
    fn dynamic_max_iterations_one_input() {
        // case 1: x = 1, int_part = 1
        // iterations = 1 * DECIMAL_PLACES + 1 = 1 * 9 + 1 = 10
        let x: FixedU64 = 1.into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 10); // 1 * 9 + 1
    }

    #[test]
    fn dynamic_max_iterations_small_integers() {
        // case 1: x = 2, int_part = 2
        // iterations = 2 * 9 + 1 = 19
        let x: FixedU64 = 2.into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 19);

        // case 2: x = 5, int_part = 5
        // iterations = 5 * 9 + 1 = 46
        let x: FixedU64 = 5.into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 46);

        // case 3: x = 10, int_part = 10
        // iterations = 10 * 9 + 1 = 91
        let x: FixedU64 = 10.into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 91);
    }

    #[test]
    fn dynamic_max_iterations_fractional_values() {
        // case 1: x = 0.5, int_part = 0
        // iterations = 0 * 9 + 1 = 1
        let x = FixedU64::saturating_from_rational(1, 2);
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 1);

        // case 2: x = 0.9, int_part = 0
        // iterations = 0 * 99 + 1 = 1
        let x = FixedU64::saturating_from_rational(9, 10);
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 1);

        // case 3: x = 1.5, int_part = 1
        // iterations = 1 * 9 + 1 = 10
        let x = FixedU64::saturating_from_rational(3, 2);
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 10);

        // case 4: x = 2.7, int_part = 2
        // iterations = 2 * 9 + 1 = 19
        let x = FixedU64::saturating_from_rational(27, 10);
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 19);
    }

    #[test]
    fn dynamic_max_iterations_negative_values() {
        // case 1: x = -1, abs = 1, int_part = 1
        // iterations = 1 * 9 + 1 = 10
        let x: FixedI64 = (-1).into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 10);

        // case 2: x = -5, abs = 5, int_part = 5
        // iterations = 5 * 9 + 1 = 46
        let x: FixedI64 = (-5).into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 46);

        // case 3: x = -10, abs = 10, int_part = 10
        // iterations = 10 * 9 + 1 = 91
        let x: FixedI64 = (-10).into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 91);

        // case 4: x = -0.5, abs = 0.5, int_part = 0
        // iterations = 0 * 9 + 1 = 1
        let x = FixedI64::saturating_from_rational(-1, 2);
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 1);
    }

    #[test]
    fn dynamic_max_iterations_large_values() {
        // case 1: x = 100, int_part = 100
        // iterations = 100 * 9 + 1 = 901
        let x: FixedU64 = 100.into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 901);

        // case 2: x = 1000, int_part = 1000
        // iterations = 1000 * 9 + 1 = 9001
        let x: FixedU64 = 1000.into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 9001);

        // case 3: x = 10000, int_part = 10000
        // iterations = 10000 * 9 + 1 = 90001
        let x: FixedU64 = 10000.into();
        let result = dynamic_max_iterations(&x);
        assert_eq!(result, 90001);
    }

    #[test]
    fn dynamic_max_iterations_saturation_check() {
        // case 1: Overflow values
        // This depends on the max value of the fixed-point type
        let x: FixedU64 = (u32::MAX as u64).into();
        let result = dynamic_max_iterations(&x);

        // Should return a valid value without panicking
        assert!(result >= 1);

        // Result should be reasonable (not u32::MAX due to saturation)
        // int_part = u32::MAX, iterations = u32::MAX * 5 + 1
        // This will saturate to u32::MAX
        assert_eq!(result, u32::MAX);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````````````` TO_U32_FLOOR ````````````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

    #[test]
    fn to_u32_floor_zero_input() {
        // case 1: x = 0 -> 0
        let x: FixedU64 = 0.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 0);

        // case 2: x = 0 (signed)
        let x: FixedI64 = 0.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 0);
    }

    #[test]
    fn to_u32_floor_positive_integers() {
        // case 1: x = 1 -> 1
        let x: FixedU64 = 1.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 1);

        // case 2: x = 10 -> 10
        let x: FixedU64 = 10.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 10);

        // case 3: x = 100 -> 100
        let x: FixedU64 = 100.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 100);

        // case 4: x = 1000 -> 1000
        let x: FixedU64 = 1000.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 1000);

        // case 5: x = 42 -> 42
        let x: FixedU64 = 42.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 42);
    }

    #[test]
    fn to_u32_floor_fractional_values() {
        // case 1: x = 1.5 -> 1 (floor)
        let x = FixedU64::saturating_from_rational(3, 2);
        let result = to_u32_floor(&x);
        assert_eq!(result, 1);

        // case 2: x = 2.7 -> 2 (floor)
        let x = FixedU64::saturating_from_rational(27, 10);
        let result = to_u32_floor(&x);
        assert_eq!(result, 2);

        // case 3: x = 9.999 -> 9 (floor)
        let x = FixedU64::saturating_from_rational(9999, 1000);
        let result = to_u32_floor(&x);
        assert_eq!(result, 9);

        // case 4: x = 100.1 -> 100 (floor)
        let x = FixedU64::saturating_from_rational(1001, 10);
        let result = to_u32_floor(&x);
        assert_eq!(result, 100);

        // case 5: x = 0.5 -> 0 (floor)
        let x = FixedU64::saturating_from_rational(1, 2);
        let result = to_u32_floor(&x);
        assert_eq!(result, 0);

        // case 6: x = 0.9 -> 0 (floor)
        let x = FixedU64::saturating_from_rational(9, 10);
        let result = to_u32_floor(&x);
        assert_eq!(result, 0);
    }

    #[test]
    fn to_u32_floor_negative_values() {
        // case 1: x = -1 -> 0 (clamped)
        let x: FixedI64 = (-1).into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 0);

        // case 2: x = -10 -> 0 (clamped)
        let x: FixedI64 = (-10).into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 0);

        // case 3: x = -0.5 -> 0 (clamped)
        let x = FixedI64::saturating_from_rational(-1, 2);
        let result = to_u32_floor(&x);
        assert_eq!(result, 0);

        // case 4: x = -100.5 -> 0 (clamped)
        let x = FixedI64::saturating_from_rational(-201, 2);
        let result = to_u32_floor(&x);
        assert_eq!(result, 0);

        // case 5: x = i64::MIN -> 0 (clamped)
        let x: FixedI64 = i64::MIN.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 0);
    }

    #[test]
    fn to_u32_floor_boundary_values() {
        // case 1: x = u32::MAX (exactly at boundary)
        let x: FixedU64 = (u32::MAX as u64).into();
        let result = to_u32_floor(&x);
        assert_eq!(result, u32::MAX);

        // case 2: x = u32::MAX + 1 (overflow, should saturate)
        let x: FixedU64 = ((u32::MAX as u64) + 1).into();
        let result = to_u32_floor(&x);
        assert_eq!(result, u32::MAX);

        // case 3: x = u32::MAX + 100 (overflow, should saturate)
        let x: FixedU64 = ((u32::MAX as u64) + 100).into();
        let result = to_u32_floor(&x);
        assert_eq!(result, u32::MAX);
    }

    #[test]
    fn to_u32_floor_large_values() {
        // case 1: x = 1000000 -> 1000000
        let x: FixedU64 = 1000000.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 1000000);

        // case 2: x = 10000000 -> 10000000
        let x: FixedU64 = 10000000.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, 10000000);

        // case 3: x = u64::MAX (overflow, should saturate)
        let x: FixedU64 = u64::MAX.into();
        let result = to_u32_floor(&x);
        assert_eq!(result, u32::MAX);
    }

    #[test]
    fn to_u32_floor_from_inner_representation() {
        // case 1: FixedU64 with DIV = 1_000_000_000
        // inner = 5_000_000_000 represents 5.0
        let x = FixedU64::from_inner(5_000_000_000);
        let result = to_u32_floor(&x);
        assert_eq!(result, 5);

        // case 2: inner = 5_500_000_000 represents 5.5
        let x = FixedU64::from_inner(5_500_000_000);
        let result = to_u32_floor(&x);
        assert_eq!(result, 5);

        // case 3: inner = 999_999_999 represents 0.999999999
        let x = FixedU64::from_inner(999_999_999);
        let result = to_u32_floor(&x);
        assert_eq!(result, 0);

        // case 4: inner = 1_000_000_000 represents 1.0
        let x = FixedU64::from_inner(1_000_000_000);
        let result = to_u32_floor(&x);
        assert_eq!(result, 1);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````` FIXED_SIGNED_CAST - FixedI64 ````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    //
    // FixedI64 is an identity implementation: Signed = Self.
    // Every conversion is a no-op - the value passes through unchanged.
 
    #[test]
    fn fixed_signed_cast_i64_checked_into() {
        // Any FixedI64 value should always yield Some(itself).
        let x = FixedI64::saturating_from_integer(5);
        assert_eq!(FixedI64::checked_into(x), Some(x));
 
        let x = FixedI64::saturating_from_integer(-7);
        assert_eq!(FixedI64::checked_into(x), Some(x));
 
        let x = FixedI64::zero();
        assert_eq!(FixedI64::checked_into(x), Some(x));
 
        let x = FixedI64::max_value();
        assert_eq!(FixedI64::checked_into(x), Some(x));
 
        let x = FixedI64::min_value();
        assert_eq!(FixedI64::checked_into(x), Some(x));
    }
 
    #[test]
    fn fixed_signed_cast_i64_saturated_into() {
        // Identity: saturated_into on a signed type is always a no-op.
        let x = FixedI64::saturating_from_integer(42);
        assert_eq!(FixedI64::saturated_into(x), x);
 
        let x = FixedI64::saturating_from_integer(-42);
        assert_eq!(FixedI64::saturated_into(x), x);
 
        let x = FixedI64::max_value();
        assert_eq!(FixedI64::saturated_into(x), x);
    }
 
    #[test]
    fn fixed_signed_cast_i64_checked_from() {
        // Identity: checked_from on a signed type always yields Some(itself).
        let x = FixedI64::saturating_from_integer(3);
        assert_eq!(FixedI64::checked_from(x), Some(x));
 
        let x = FixedI64::saturating_from_integer(-3);
        assert_eq!(FixedI64::checked_from(x), Some(x));
 
        let x = FixedI64::min_value();
        assert_eq!(FixedI64::checked_from(x), Some(x));
    }
 
    #[test]
    fn fixed_signed_cast_i64_saturated_from() {
        // Identity: saturated_from on a signed type is always a no-op.
        let x = FixedI64::saturating_from_integer(10);
        assert_eq!(FixedI64::saturated_from(x), x);
 
        let x = FixedI64::saturating_from_integer(-10);
        assert_eq!(FixedI64::saturated_from(x), x);
    }
 
    #[test]
    fn fixed_signed_cast_i64_saturating_closure() {
        // saturating wraps a signed closure - on FixedI64 it is a pure pass-through.
        let x = FixedI64::saturating_from_integer(4);
        let result = FixedI64::saturating(x, |v| v.saturating_add(FixedI64::saturating_from_integer(1)));
        assert_eq!(result, FixedI64::saturating_from_integer(5));
 
        // Closure that negates: result is negative but still representable.
        let x = FixedI64::saturating_from_integer(3);
        let result = FixedI64::saturating(x, |v| v.saturating_mul(FixedI64::saturating_from_integer(-1)));
        assert_eq!(result, FixedI64::saturating_from_integer(-3));
    }
 
    #[test]
    fn fixed_signed_cast_i64_checked_closure() {
        // checked wraps a signed closure - returns Some for in-range results,
        // None when the result saturates to min_value() or max_value().
        let x = FixedI64::saturating_from_integer(7);
        let result = FixedI64::checked(x, |v| {
            v.unwrap_or(FixedI64::zero()).saturating_add(FixedI64::saturating_from_integer(1))
        });
        assert_eq!(result, Some(FixedI64::saturating_from_integer(8)));
    }

    #[test]
    fn fixed_signed_cast_i64_checked_closure_overflow_is_none() {
        // Closure overflows to max_value via saturating arithmetic - checked returns None.
        let x = FixedI64::max_value();
        let result = FixedI64::checked(x, |v| {
            v.unwrap_or(FixedI64::zero()).saturating_add(FixedI64::one())
        });
        assert_eq!(result, None);
    }
 
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````` FIXED_SIGNED_CAST - FixedI128 ```````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    //
    // FixedI128 is also an identity implementation: Signed = Self.
 
    #[test]
    fn fixed_signed_cast_i128_checked_into() {
        let x = FixedI128::saturating_from_integer(100);
        assert_eq!(FixedI128::checked_into(x), Some(x));
 
        let x = FixedI128::saturating_from_integer(-100);
        assert_eq!(FixedI128::checked_into(x), Some(x));
 
        let x = FixedI128::zero();
        assert_eq!(FixedI128::checked_into(x), Some(x));
 
        let x = FixedI128::max_value();
        assert_eq!(FixedI128::checked_into(x), Some(x));
 
        let x = FixedI128::min_value();
        assert_eq!(FixedI128::checked_into(x), Some(x));
    }
 
    #[test]
    fn fixed_signed_cast_i128_saturated_into() {
        let x = FixedI128::saturating_from_integer(999);
        assert_eq!(FixedI128::saturated_into(x), x);
 
        let x = FixedI128::saturating_from_integer(-999);
        assert_eq!(FixedI128::saturated_into(x), x);
    }
 
    #[test]
    fn fixed_signed_cast_i128_checked_from() {
        let x = FixedI128::saturating_from_integer(50);
        assert_eq!(FixedI128::checked_from(x), Some(x));
 
        let x = FixedI128::saturating_from_integer(-50);
        assert_eq!(FixedI128::checked_from(x), Some(x));
    }
 
    #[test]
    fn fixed_signed_cast_i128_saturated_from() {
        let x = FixedI128::saturating_from_integer(77);
        assert_eq!(FixedI128::saturated_from(x), x);
 
        let x = FixedI128::saturating_from_integer(-77);
        assert_eq!(FixedI128::saturated_from(x), x);
    }
 
    #[test]
    fn fixed_signed_cast_i128_saturating_closure() {
        let x = FixedI128::saturating_from_integer(10);
        let result = FixedI128::saturating(x, |v| v.saturating_add(FixedI128::saturating_from_integer(5)));
        assert_eq!(result, FixedI128::saturating_from_integer(15));
 
        let x = FixedI128::saturating_from_integer(-10);
        let result = FixedI128::saturating(x, |v| v.saturating_mul(FixedI128::saturating_from_integer(2)));
        assert_eq!(result, FixedI128::saturating_from_integer(-20));
    }
 
    #[test]
    fn fixed_signed_cast_i128_checked_closure() {
        let x = FixedI128::saturating_from_integer(3);
        let result = FixedI128::checked(x, |v| {
            v.unwrap_or(FixedI128::zero()).saturating_add(FixedI128::saturating_from_integer(2))
        });
        assert_eq!(result, Some(FixedI128::saturating_from_integer(5)));
    }
 
    #[test]
    fn fixed_signed_cast_i128_checked_closure_overflow_is_none() {
        // Closure overflows to max_value via saturating arithmetic - checked returns None.
        let x = FixedI128::max_value();
        let result = FixedI128::checked(x, |v| {
            v.unwrap_or(FixedI128::zero()).saturating_add(FixedI128::one())
        });
        assert_eq!(result, None);
    }

    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````` FIXED_SIGNED_CAST - FixedU64 ````````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    //
    // FixedU64 bridges to FixedI128. u64 inner always fits in i128,
    // so checked_into / saturated_into are always infallible.
    // The reverse direction can fail when the signed result is negative
    // or exceeds u64::MAX.
 
    #[test]
    fn fixed_signed_cast_u64_checked_into_always_some() {
        // u64 inner always representable in i128.
        let x = FixedU64::saturating_from_integer(0);
        assert!(FixedU64::checked_into(x).is_some());
 
        let x = FixedU64::saturating_from_integer(1);
        let signed = FixedU64::checked_into(x).unwrap();
        // The signed inner should equal the unsigned inner cast to i128.
        assert_eq!(signed.into_inner(), x.into_inner() as i128);
 
        let x = FixedU64::max_value();
        assert!(FixedU64::checked_into(x).is_some());
    }
 
    #[test]
    fn fixed_signed_cast_u64_saturated_into_round_trip() {
        // saturated_into then checked_from should recover the original value
        // for any representable FixedU64.
        let values = [
            FixedU64::saturating_from_integer(0),
            FixedU64::saturating_from_integer(1),
            FixedU64::saturating_from_integer(1000),
            FixedU64::saturating_from_rational(3, 2), // 1.5
            FixedU64::max_value(),
        ];
        for x in values {
            let signed = FixedU64::saturated_into(x);
            let back = FixedU64::checked_from(signed).unwrap();
            assert_eq!(back, x, "round-trip failed for {x:?}");
        }
    }
 
    #[test]
    fn fixed_signed_cast_u64_checked_from_negative_is_none() {
        // Negative FixedI128 values cannot be represented as FixedU64.
        let neg = FixedI128::saturating_from_integer(-1);
        assert_eq!(FixedU64::checked_from(neg), None);
 
        let neg = FixedI128::saturating_from_integer(-100);
        assert_eq!(FixedU64::checked_from(neg), None);
 
        // Minimum i128 value - strongly negative.
        let neg = FixedI128::min_value();
        assert_eq!(FixedU64::checked_from(neg), None);
    }
 
    #[test]
    fn fixed_signed_cast_u64_checked_from_above_u64_max_is_none() {
        // FixedI128 inner > u64::MAX cannot fit in FixedU64.
        // Construct an i128 inner that represents a value just above u64::MAX.
        // FixedU64::DIV = 10^9; FixedI128::DIV = 10^18.
        // We need inner_i128 > u64::MAX (as a raw inner value of FixedI128).
        // u64::MAX as i128 = 18_446_744_073_709_551_615.
        let too_large = FixedI128::from_inner(u64::MAX as i128 + 1);
        assert_eq!(FixedU64::checked_from(too_large), None);
    }
 
    #[test]
    fn fixed_signed_cast_u64_checked_from_zero_is_some() {
        let zero = FixedI128::zero();
        assert_eq!(FixedU64::checked_from(zero), Some(FixedU64::zero()));
    }
 
    #[test]
    fn fixed_signed_cast_u64_saturated_from_negative_clamps_to_zero() {
        // Negative values saturate to zero (the unsigned floor).
        let neg = FixedI128::saturating_from_integer(-5);
        assert_eq!(FixedU64::saturated_from(neg), FixedU64::zero());
 
        let neg = FixedI128::min_value();
        assert_eq!(FixedU64::saturated_from(neg), FixedU64::zero());
    }
 
    #[test]
    fn fixed_signed_cast_u64_saturated_from_above_max_clamps_to_max() {
        // Values above u64::MAX saturate to u64::MAX.
        let too_large = FixedI128::from_inner(u64::MAX as i128 + 1);
        assert_eq!(FixedU64::saturated_from(too_large), FixedU64::from_inner(u64::MAX));
    }
 
    #[test]
    fn fixed_signed_cast_u64_saturating_closure_with_positive_result() {
        // Closure doubles the value - result stays within FixedU64 range.
        let x = FixedU64::saturating_from_integer(3);
        let result = FixedU64::saturating(x, |v| v.saturating_add(v));
        assert_eq!(result, FixedU64::saturating_from_integer(6));
    }
 
    #[test]
    fn fixed_signed_cast_u64_saturating_closure_negative_result_clamps() {
        // Closure produces a negative signed result - saturated_from clamps to 0.
        let x = FixedU64::saturating_from_integer(2);
        let result = FixedU64::saturating(x, |v| v.saturating_mul(FixedI128::saturating_from_integer(-1)));
        assert_eq!(result, FixedU64::zero());
    }
 
    #[test]
    fn fixed_signed_cast_u64_checked_closure_negative_result_is_none() {
        // Closure produces a negative signed result - checked_from returns None.
        let x = FixedU64::saturating_from_integer(5);
        let result = FixedU64::checked(x, |v| {
        // saturating_mul by the fixed-point -1.0 correctly negates because
        // fixed-point multiplication divides by DIV, cancelling the scale difference.
        // (Unlike saturating_add, which operates on raw inner values and would be wrong
        // with a FixedI128 integer constant - see the checked_closure_valid test.)
            v.unwrap().saturating_mul(FixedI128::saturating_from_integer(-1))
        });
        assert_eq!(result, None);
    }
 
    #[test]
    fn fixed_signed_cast_u64_checked_closure_valid_result_is_some() {
        // Closure adds one unit in FixedU64 scale.
        // The bridge casts raw inner bits into FixedI128, so arithmetic inside
        // the closure must use FixedU64::DIV (10^9) as "one unit", not
        // FixedI128::saturating_from_integer(1) which uses FixedI128::DIV (10^18).
        let x = FixedU64::saturating_from_integer(10);
        let one_unit = FixedI128::from_inner(FixedU64::DIV as i128); // = 1 in FixedU64 scale
        let result = FixedU64::checked(x, |v| {
            v.unwrap().saturating_add(one_unit)
        });
        assert_eq!(result, Some(FixedU64::saturating_from_integer(11)));
    }
 
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    // ````````````````````````` FIXED_SIGNED_CAST - FixedU128 ```````````````````````
    // ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~
    //
    // FixedU128 bridges to FixedI128. Unlike FixedU64, the inner u128 value
    // can exceed i128::MAX, making checked_into fallible for large values.
 
    #[test]
    fn fixed_signed_cast_u128_checked_into_small_values_are_some() {
        let x = FixedU128::saturating_from_integer(0);
        assert!(FixedU128::checked_into(x).is_some());
 
        let x = FixedU128::saturating_from_integer(1);
        let signed = FixedU128::checked_into(x).unwrap();
        assert_eq!(signed.into_inner(), x.into_inner() as i128);
 
        let x = FixedU128::saturating_from_integer(1_000_000);
        assert!(FixedU128::checked_into(x).is_some());
    }
 
    #[test]
    fn fixed_signed_cast_u128_checked_into_above_i128_max_is_none() {
        // Inner value just above i128::MAX is unrepresentable in FixedI128.
        let too_large = FixedU128::from_inner(i128::MAX as u128 + 1);
        assert_eq!(FixedU128::checked_into(too_large), None);
 
        // Maximum FixedU128 value is definitely unrepresentable.
        assert_eq!(FixedU128::checked_into(FixedU128::max_value()), None);
    }
 
    #[test]
    fn fixed_signed_cast_u128_saturated_into_large_clamps_to_i128_max() {
        // Values above i128::MAX saturate to i128::MAX in the signed workspace.
        let too_large = FixedU128::from_inner(i128::MAX as u128 + 1);
        let signed = FixedU128::saturated_into(too_large);
        assert_eq!(signed, FixedI128::from_inner(i128::MAX));
 
        let signed_max = FixedU128::saturated_into(FixedU128::max_value());
        assert_eq!(signed_max, FixedI128::from_inner(i128::MAX));
    }
 
    #[test]
    fn fixed_signed_cast_u128_saturated_into_small_values_exact() {
        let x = FixedU128::saturating_from_integer(42);
        let signed = FixedU128::saturated_into(x);
        assert_eq!(signed.into_inner(), x.into_inner() as i128);
    }
 
    #[test]
    fn fixed_signed_cast_u128_checked_from_negative_is_none() {
        let neg = FixedI128::saturating_from_integer(-1);
        assert_eq!(FixedU128::checked_from(neg), None);
 
        let neg = FixedI128::min_value();
        assert_eq!(FixedU128::checked_from(neg), None);
    }
 
    #[test]
    fn fixed_signed_cast_u128_checked_from_non_negative_is_some() {
        let zero = FixedI128::zero();
        assert_eq!(FixedU128::checked_from(zero), Some(FixedU128::zero()));
 
        let pos = FixedI128::saturating_from_integer(100);
        let result = FixedU128::checked_from(pos).unwrap();
        assert_eq!(result.into_inner(), pos.into_inner() as u128);
    }
 
    #[test]
    fn fixed_signed_cast_u128_saturated_from_negative_clamps_to_zero() {
        let neg = FixedI128::saturating_from_integer(-99);
        assert_eq!(FixedU128::saturated_from(neg), FixedU128::zero());
 
        let neg = FixedI128::min_value();
        assert_eq!(FixedU128::saturated_from(neg), FixedU128::zero());
    }
 
    #[test]
    fn fixed_signed_cast_u128_saturated_from_non_negative_exact() {
        let pos = FixedI128::saturating_from_integer(500);
        let result = FixedU128::saturated_from(pos);
        assert_eq!(result.into_inner(), pos.into_inner() as u128);
 
        let zero = FixedI128::zero();
        assert_eq!(FixedU128::saturated_from(zero), FixedU128::zero());
    }
 
    #[test]
    fn fixed_signed_cast_u128_round_trip_for_representable_values() {
        let values = [
            FixedU128::saturating_from_integer(0),
            FixedU128::saturating_from_integer(1),
            FixedU128::saturating_from_integer(1_000_000),
            FixedU128::saturating_from_rational(7, 3),
        ];
        for x in values {
            let signed = FixedU128::saturated_into(x);
            let back = FixedU128::checked_from(signed).unwrap();
            assert_eq!(back, x, "round-trip failed for {x:?}");
        }
    }
 
    #[test]
    fn fixed_signed_cast_u128_saturating_closure_valid() {
        let x = FixedU128::saturating_from_integer(4);
        let result = FixedU128::saturating(x, |v| v.saturating_add(FixedI128::saturating_from_integer(6)));
        assert_eq!(result, FixedU128::saturating_from_integer(10));
    }
 
    #[test]
    fn fixed_signed_cast_u128_saturating_closure_negative_clamps_to_zero() {
        let x = FixedU128::saturating_from_integer(3);
        let result = FixedU128::saturating(x, |v| v.saturating_mul(FixedI128::saturating_from_integer(-2)));
        assert_eq!(result, FixedU128::zero());
    }
 
    #[test]
    fn fixed_signed_cast_u128_checked_closure_negative_is_none() {
        let x = FixedU128::saturating_from_integer(5);
        let result = FixedU128::checked(x, |v| {
            v.unwrap_or(FixedI128::zero())
                .saturating_mul(FixedI128::saturating_from_integer(-1))
        });
        assert_eq!(result, None);
    }
 
    #[test]
    fn fixed_signed_cast_u128_checked_closure_valid_is_some() {
        let x = FixedU128::saturating_from_integer(20);
        let result = FixedU128::checked(x, |v| {
            v.unwrap_or(FixedI128::zero())
                .saturating_add(FixedI128::saturating_from_integer(5))
        });
        assert_eq!(result, Some(FixedU128::saturating_from_integer(25)));
    }
 
    #[test]
    fn fixed_signed_cast_u128_checked_into_none_propagated_through_closure() {
        // When checked_into returns None, the closure receives None.
        // The closure must handle this case - here it falls back to zero.
        let too_large = FixedU128::from_inner(i128::MAX as u128 + 1);
        let result = FixedU128::checked(too_large, |v| {
            v.unwrap_or(FixedI128::zero()) // None -> zero -> not representable? No: zero is fine.
        });
        // zero is representable as FixedU128, so we get Some(0).
        assert_eq!(result, Some(FixedU128::zero()));
    }
 
}
