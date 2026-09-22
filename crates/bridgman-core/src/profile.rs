//! Checked quantities for the thermal slice. Declared unit transforms are exact
//! rationals; evaluated magnitudes are finite binary64, not exact measurements.
//!
//! ```
//! use bridgman_core::profile::*;
//! let capacity = (KILOGRAM.quantity(2.0)? * JOULE_PER_KG_K.quantity(500.0)?)?;
//! let heat = (capacity * KELVIN_DELTA.quantity(100.0)?)?;
//! assert_eq!(heat.in_unit(JOULE)?, 100000.0);
//! # Ok::<(), QuantityError>(())
//! ```
//! ```compile_fail
//! use bridgman_core::profile::*;
//! let invalid = JOULE.quantity(1.0).unwrap() + NEWTON_METRE.quantity(1.0).unwrap();
//! ```
//! ```compile_fail
//! use bridgman_core::profile::*;
//! let invalid = CELSIUS.quantity(20.0).unwrap() + KELVIN.quantity(300.0).unwrap();
//! ```
//! ```compile_fail
//! use bridgman_core::profile::*;
//! let invalid = KELVIN.quantity(300.0).unwrap().scale(2.0);
//! ```
use std::cmp::Ordering;
use std::marker::PhantomData;
use std::ops::{Add, Div, Mul, Sub};

use crate::Dimensions;

/// The binary operations of the expression language and of quantity arithmetic.
/// Authored expressions, kind rules and numerical evaluation share this one type.
pub use crate::Op;
/// Any checked quantity operation, as named in an unsupported-operation error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Binary(Op),
    Scale,
    DivideScalar,
    Sqrt,
    Abs,
}
mod sealed {
    pub trait Sealed {}
}
pub trait QuantityKind: sealed::Sealed + Copy + std::fmt::Debug + PartialEq {
    const KIND: Kind;
}
pub trait Linear: QuantityKind {}
/// Each profile kind is named once: this declares the dynamic `Kind` tag, its
/// dimensions and the typed marker together.
macro_rules! kinds {
    ($($name:ident { $($base:literal: $power:literal),* }),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum Kind {
            $($name),*
        }
        impl Kind {
            pub fn dimensions(self) -> Dimensions {
                let powers: &[(&str, i64)] = match self {
                    $(Self::$name => &[$(($base, $power)),*]),*
                };
                Dimensions::from_integer_powers(powers.iter().copied())
            }
        }
        $(
            #[derive(Clone, Copy, Debug, PartialEq)]
            pub struct $name;
            impl sealed::Sealed for $name {}
            impl QuantityKind for $name {
                const KIND: Kind = Kind::$name;
            }
        )*
    };
}
/// The linear kinds, named once for both the typed `Linear` marker and the
/// dynamic check.
macro_rules! linear {
    ($($name:ident),* $(,)?) => {
        $(impl Linear for $name {})*
        fn kind_is_linear(kind: Kind) -> bool {
            matches!(kind, $(Kind::$name)|*)
        }
    };
}
include!("profile_kinds.rs");

#[derive(Clone, Debug, PartialEq)]
pub enum QuantityError {
    NonFiniteInput,
    NumericalFailure,
    BelowAbsoluteZero,
    DivisionByZero,
    NegativeRoot,
    KindMismatch {
        expected: Kind,
        actual: Kind,
    },
    UnknownUnit(String),
    UnsupportedOperation {
        operation: Operation,
        left: Kind,
        right: Option<Kind>,
    },
}
impl std::fmt::Display for QuantityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFiniteInput => write!(f, "quantity input must be finite"),
            Self::NumericalFailure => write!(
                f,
                "quantity arithmetic or conversion produced a nonfinite value"
            ),
            Self::BelowAbsoluteZero => write!(f, "absolute temperature cannot be below 0 K"),
            Self::DivisionByZero => write!(f, "quantity division requires a nonzero denominator"),
            Self::NegativeRoot => write!(f, "real square root requires a nonnegative area"),
            Self::KindMismatch { expected, actual } => {
                write!(f, "expected {expected:?}, received {actual:?}")
            }
            Self::UnknownUnit(u) => write!(f, "unsupported unit {u:?}"),
            Self::UnsupportedOperation {
                operation,
                left,
                right,
            } => write!(f, "unsupported {operation:?} for {left:?} and {right:?}"),
        }
    }
}
impl std::error::Error for QuantityError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rational {
    pub numerator: i64,
    pub denominator: i64,
}
#[derive(Clone, Copy, Debug)]
pub struct Unit<K: QuantityKind> {
    symbol: &'static str,
    scale: Rational,
    offset: Rational,
    marker: PhantomData<K>,
}
impl<K: QuantityKind> Unit<K> {
    const fn new(symbol: &'static str, n: i64, d: i64, on: i64, od: i64) -> Self {
        Self {
            symbol,
            scale: Rational {
                numerator: n,
                denominator: d,
            },
            offset: Rational {
                numerator: on,
                denominator: od,
            },
            marker: PhantomData,
        }
    }
    pub const fn symbol(self) -> &'static str {
        self.symbol
    }
    pub const fn scale(self) -> Rational {
        self.scale
    }
    /// Canonical offset in q_SI = scale * q_display + offset.
    pub const fn offset(self) -> Rational {
        self.offset
    }
    fn before_scale(self) -> f64 {
        // Catalog-only constants: no user-provided integer products.
        (self.offset.numerator as i128 * self.scale.denominator as i128) as f64
            / (self.offset.denominator as i128 * self.scale.numerator as i128) as f64
    }
    pub fn quantity(self, value: f64) -> Result<Quantity<K>, QuantityError> {
        if !value.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        let canonical = (value + self.before_scale())
            * (self.scale.numerator as f64 / self.scale.denominator as f64);
        Quantity::computed(canonical)
    }
}
/// The unit catalog. Typed constants and the dynamic symbol boundary are both
/// generated here, so a unit cannot exist in one and be unknown to the other.
macro_rules! units {
    ($($constant:ident:$kind:ident=$symbol:literal,$n:literal,$d:literal,$on:literal,$od:literal);* $(;)?)=>{
        $(pub const $constant:Unit<$kind>=Unit::new($symbol,$n,$d,$on,$od);)*
        impl AnyQuantity {
            /// Document/caller boundary: parse a declared unit symbol once.
            pub fn from_unit(value: f64, unit: &str) -> Result<Self, QuantityError> {
                match unit {
                    $($symbol => $constant.quantity(value).map(Into::into),)*
                    unknown => Err(QuantityError::UnknownUnit(unknown.into())),
                }
            }
            pub fn in_unit(self, unit: &str) -> Result<f64, QuantityError> {
                match unit {
                    $($symbol => self.try_typed()?.in_unit($constant),)*
                    unknown => Err(QuantityError::UnknownUnit(unknown.into())),
                }
            }
        }
    };
}
include!("profile_units.rs");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity<K: QuantityKind> {
    canonical: f64,
    marker: PhantomData<K>,
}
impl<K: QuantityKind> Quantity<K> {
    fn computed(value: f64) -> Result<Self, QuantityError> {
        checked(K::KIND, value)?;
        Ok(Self {
            canonical: value,
            marker: PhantomData,
        })
    }
    pub fn in_unit(self, unit: Unit<K>) -> Result<f64, QuantityError> {
        let value = self.canonical / (unit.scale.numerator as f64 / unit.scale.denominator as f64)
            - unit.before_scale();
        if value.is_finite() {
            Ok(value)
        } else {
            Err(QuantityError::NumericalFailure)
        }
    }
    pub fn kind(self) -> Kind {
        K::KIND
    }
}
/// Same-kind canonical magnitudes share a scale, so their order is meaningful.
impl<K: QuantityKind> PartialOrd for Quantity<K> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.canonical.partial_cmp(&other.canonical)
    }
}
impl<K: Linear> Quantity<K> {
    pub fn scale(self, scalar: f64) -> Result<Self, QuantityError> {
        if !scalar.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        Self::computed(self.canonical * scalar)
    }
    pub fn divide_scalar(self, scalar: f64) -> Result<Self, QuantityError> {
        if !scalar.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        if scalar == 0.0 {
            return Err(QuantityError::DivisionByZero);
        }
        Self::computed(self.canonical / scalar)
    }
}
/// Dynamic boundary for authored inputs. Fields are private so invariants cannot
/// be bypassed by deserializing a canonical magnitude or arbitrary kind tag.
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(try_from = "Stated")]
pub struct AnyQuantity {
    kind: Kind,
    canonical: f64,
}

/// A number and unit symbol as written, not yet checked. Declarations convert
/// it while parsing; a question keeps it so that a nonfinite or unknown-unit
/// input is reported as that question's outcome instead of a document error.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stated {
    pub value: f64,
    pub unit: String,
}
/// One variable's values along a trace, as written: a shared unit symbol and
/// one number per sample. Checked like `Stated`, when the question is asked.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub unit: String,
    pub values: Vec<f64>,
}
impl Series {
    pub fn quantities(&self) -> Result<Vec<AnyQuantity>, QuantityError> {
        self.values
            .iter()
            .map(|value| AnyQuantity::from_unit(*value, &self.unit))
            .collect()
    }
}
impl TryFrom<Stated> for AnyQuantity {
    type Error = QuantityError;
    fn try_from(input: Stated) -> Result<Self, Self::Error> {
        Self::from_unit(input.value, &input.unit)
    }
}
impl<'de, K: QuantityKind> serde::Deserialize<'de> for Quantity<K> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        AnyQuantity::deserialize(d)?
            .try_typed()
            .map_err(serde::de::Error::custom)
    }
}
impl<K: QuantityKind> From<Quantity<K>> for AnyQuantity {
    fn from(q: Quantity<K>) -> Self {
        Self {
            kind: K::KIND,
            canonical: q.canonical,
        }
    }
}
impl AnyQuantity {
    pub fn scale(self, scalar: f64) -> Result<Self, QuantityError> {
        if !kind_is_linear(self.kind) {
            return Err(QuantityError::UnsupportedOperation {
                operation: Operation::Scale,
                left: self.kind,
                right: None,
            });
        }
        if !scalar.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        let canonical = self.canonical * scalar;
        checked(self.kind, canonical)?;
        Ok(Self { canonical, ..self })
    }
    pub fn divide_scalar(self, scalar: f64) -> Result<Self, QuantityError> {
        if !kind_is_linear(self.kind) {
            return Err(QuantityError::UnsupportedOperation {
                operation: Operation::DivideScalar,
                left: self.kind,
                right: None,
            });
        }
        if !scalar.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        let canonical = quotient(self.canonical, scalar)?;
        checked(self.kind, canonical)?;
        Ok(Self { canonical, ..self })
    }
    pub fn sqrt(self) -> Result<Self, QuantityError> {
        sqrt_dynamic(self)
    }
    pub fn kind(self) -> Kind {
        self.kind
    }
    pub fn try_typed<K: QuantityKind>(self) -> Result<Quantity<K>, QuantityError> {
        if self.kind != K::KIND {
            return Err(QuantityError::KindMismatch {
                expected: K::KIND,
                actual: self.kind,
            });
        }
        Quantity::computed(self.canonical)
    }
    /// Every dynamic binary operation, authored or direct, ends here.
    pub fn apply(self, op: Op, b: Self) -> Result<Self, QuantityError> {
        let kind = binary_kind(self.kind, b.kind, op)?;
        let canonical = arithmetic(op, self.canonical, b.canonical)?;
        checked(kind, canonical)?;
        Ok(Self { kind, canonical })
    }
    /// The order of two magnitudes of one kind; different kinds have none.
    pub fn compare(self, other: Self) -> Result<Ordering, QuantityError> {
        self.same_kind(other)?;
        self.canonical
            .partial_cmp(&other.canonical)
            .ok_or(QuantityError::NumericalFailure)
    }
    /// The magnitude with its sign dropped. Like scaling, it is defined for
    /// linear kinds only.
    pub fn abs(self) -> Result<Self, QuantityError> {
        if !kind_is_linear(self.kind) {
            return Err(QuantityError::UnsupportedOperation {
                operation: Operation::Abs,
                left: self.kind,
                right: None,
            });
        }
        Ok(Self {
            canonical: self.canonical.abs(),
            ..self
        })
    }
    pub fn is_zero(self) -> bool {
        self.canonical == 0.0
    }
    /// Whether `|self| <= tolerance`, for a tolerance of the same kind.
    pub fn within(self, tolerance: Self) -> Result<bool, QuantityError> {
        self.same_kind(tolerance)?;
        Ok(self.abs()?.canonical <= tolerance.canonical)
    }
    fn same_kind(self, other: Self) -> Result<(), QuantityError> {
        if self.kind == other.kind {
            Ok(())
        } else {
            Err(QuantityError::KindMismatch {
                expected: self.kind,
                actual: other.kind,
            })
        }
    }
    /// Escape hatch kept only until Physica moves to the typed operations
    /// above (`compare`, `abs`, `is_zero`, `within`).
    #[doc(hidden)]
    pub fn canonical(self) -> f64 {
        self.canonical
    }
}

/// The numeric interior shared by typed and dynamic operations. Kinds are
/// settled before it runs: by `binary_kind` for dynamic values, and by the
/// generated impls for typed ones.
fn arithmetic(op: Op, a: f64, b: f64) -> Result<f64, QuantityError> {
    match op {
        Op::Add => Ok(a + b),
        Op::Sub => Ok(a - b),
        Op::Mul => Ok(a * b),
        Op::Div => quotient(a, b),
    }
}
fn quotient(a: f64, b: f64) -> Result<f64, QuantityError> {
    if b == 0.0 {
        Err(QuantityError::DivisionByZero)
    } else {
        Ok(a / b)
    }
}
/// A typed operation whose result kind the generated profile has fixed. It
/// calls the numeric interior directly; `Quantity::computed` applies the
/// result kind's finiteness and bound checks.
macro_rules! operation {
    ($trait:ident, $method:ident, $a:ty, $b:ty, $r:ty) => {
        impl $trait<Quantity<$b>> for Quantity<$a> {
            type Output = Result<Quantity<$r>, QuantityError>;
            fn $method(self, b: Quantity<$b>) -> Self::Output {
                Quantity::computed(arithmetic(Op::$trait, self.canonical, b.canonical)?)
            }
        }
    };
}
impl<K: Linear> Add for Quantity<K> {
    type Output = Result<Self, QuantityError>;
    fn add(self, b: Self) -> Self::Output {
        Quantity::computed(arithmetic(Op::Add, self.canonical, b.canonical)?)
    }
}
impl<K: Linear> Sub for Quantity<K> {
    type Output = Result<Self, QuantityError>;
    fn sub(self, b: Self) -> Self::Output {
        Quantity::computed(arithmetic(Op::Sub, self.canonical, b.canonical)?)
    }
}
include!("profile_operations.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_comparisons_refuse_other_kinds() {
        let a: AnyQuantity = JOULE.quantity(2.0).unwrap().into();
        let b: AnyQuantity = KILOJOULE.quantity(0.001).unwrap().into();
        assert_eq!(a.compare(b), Ok(Ordering::Greater));
        let torque: AnyQuantity = NEWTON_METRE.quantity(1.0).unwrap().into();
        assert_eq!(
            a.compare(torque),
            Err(QuantityError::KindMismatch {
                expected: Kind::Energy,
                actual: Kind::Torque
            })
        );
        assert!(a.within(torque).is_err());
    }
    #[test]
    fn abs_zero_and_tolerance() {
        let residual = AnyQuantity::from_unit(-0.5, "J").unwrap();
        assert_eq!(
            residual.abs().unwrap(),
            AnyQuantity::from_unit(0.5, "J").unwrap()
        );
        assert_eq!(
            residual.within(AnyQuantity::from_unit(0.5, "J").unwrap()),
            Ok(true)
        );
        assert_eq!(
            residual.within(AnyQuantity::from_unit(0.4, "J").unwrap()),
            Ok(false)
        );
        assert!(AnyQuantity::from_unit(0.0, "J").unwrap().is_zero());
        assert!(!residual.is_zero());
        assert!(matches!(
            AnyQuantity::from_unit(1.0, "K").unwrap().abs(),
            Err(QuantityError::UnsupportedOperation {
                operation: Operation::Abs,
                ..
            })
        ));
    }
    #[test]
    fn typed_operations_keep_result_kind_checks() {
        let cold = KELVIN.quantity(1.0).unwrap();
        assert_eq!(
            cold - KELVIN_DELTA.quantity(2.0).unwrap(),
            Err(QuantityError::BelowAbsoluteZero)
        );
        assert_eq!(
            JOULE.quantity(1.0).unwrap() / KILOGRAM.quantity(0.0).unwrap(),
            Err(QuantityError::DivisionByZero)
        );
        let typed = (JOULE.quantity(3.0).unwrap() / KILOGRAM.quantity(2.0).unwrap()).unwrap();
        let dynamic = AnyQuantity::from(JOULE.quantity(3.0).unwrap())
            .apply(Op::Div, KILOGRAM.quantity(2.0).unwrap().into())
            .unwrap();
        assert_eq!(AnyQuantity::from(typed), dynamic);
    }
    #[test]
    fn kind_dimensions_share_the_dimension_type() {
        assert_eq!(Kind::Energy.dimensions(), Kind::Torque.dimensions());
        assert_eq!(Kind::Unitless.dimensions(), Dimensions::one());
        assert_eq!(
            Kind::HeatCapacity.dimensions().signature(),
            "M:1,L:2,T:-2,Theta:-1"
        );
    }
}
