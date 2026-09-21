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
use std::marker::PhantomData;
use std::ops::{Add, Div, Mul, Sub};

/// The binary operations of the expression language and of quantity arithmetic.
/// Authored expressions, kind rules and numerical evaluation share this one type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}
/// Any checked quantity operation, as named in an unsupported-operation error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Binary(Op),
    Scale,
    DivideScalar,
    Sqrt,
}
mod sealed {
    pub trait Sealed {}
}
pub trait QuantityKind: sealed::Sealed + Copy + std::fmt::Debug + PartialEq {
    const KIND: Kind;
}
pub trait Linear: QuantityKind {}
macro_rules! kinds {
    ($($name:ident),*)=>{$(
        #[derive(Clone,Copy,Debug,PartialEq)] pub struct $name;
        impl sealed::Sealed for $name {}
        impl QuantityKind for $name { const KIND:Kind=Kind::$name; }
    )*};
}
macro_rules! linear {($($name:ident),*)=>{$(impl Linear for $name {})*};}
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
    pub fn checked_add(self, b: Self) -> Result<Self, QuantityError> {
        self.apply(Op::Add, b)
    }
    pub fn subtract(self, b: Self) -> Result<Self, QuantityError> {
        self.apply(Op::Sub, b)
    }
    pub fn multiply(self, b: Self) -> Result<Self, QuantityError> {
        self.apply(Op::Mul, b)
    }
    pub fn divide(self, b: Self) -> Result<Self, QuantityError> {
        self.apply(Op::Div, b)
    }
    /// Every binary operation, typed or dynamic, authored or direct, ends here.
    pub fn apply(self, op: Op, b: Self) -> Result<Self, QuantityError> {
        let kind = binary_kind(self.kind, b.kind, op)?;
        let canonical = match op {
            Op::Add => self.canonical + b.canonical,
            Op::Sub => self.canonical - b.canonical,
            Op::Mul => self.canonical * b.canonical,
            Op::Div => quotient(self.canonical, b.canonical)?,
        };
        checked(kind, canonical)?;
        Ok(Self { kind, canonical })
    }
    #[doc(hidden)]
    pub fn canonical(self) -> f64 {
        self.canonical
    }
}

fn quotient(a: f64, b: f64) -> Result<f64, QuantityError> {
    if b == 0.0 {
        Err(QuantityError::DivisionByZero)
    } else {
        Ok(a / b)
    }
}
impl<K: Linear> Add for Quantity<K> {
    type Output = Result<Self, QuantityError>;
    fn add(self, b: Self) -> Self::Output {
        AnyQuantity::from(self).checked_add(b.into())?.try_typed()
    }
}
impl<K: Linear> Sub for Quantity<K> {
    type Output = Result<Self, QuantityError>;
    fn sub(self, b: Self) -> Self::Output {
        AnyQuantity::from(self).subtract(b.into())?.try_typed()
    }
}
macro_rules! operation {
    ($trait:ident,$method:ident,$dynamic:ident,$a:ident,$b:ident,$r:ident) => {
        impl $trait<Quantity<$b>> for Quantity<$a> {
            type Output = Result<Quantity<$r>, QuantityError>;
            fn $method(self, b: Quantity<$b>) -> Self::Output {
                AnyQuantity::from(self).$dynamic(b.into())?.try_typed()
            }
        }
    };
}
include!("profile_operations.rs");
