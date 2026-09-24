//! The one quantity: a finite value of a registry kind, held in a reference
//! unit of that kind. Every operation asks the registry which kind results.
use crate::{AffineRole, Kind, Op, Operation, ProductOp, QuantityError, Unit};
use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity<'r> {
    kind: Kind<'r>,
    /// A terminal unit of `kind`; `value` is in it.
    unit: Unit<'r>,
    value: f64,
}
/// The value in the unit it is held in, with that unit's symbol: `2 kg`, or
/// in the compact alternate form `{:#}` used inside expressions, `2[kg]`.
impl fmt::Display for Quantity<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            write!(f, "{}[{}]", self.value, self.unit.symbol())
        } else {
            write!(f, "{} {}", self.value, self.unit.symbol())
        }
    }
}

impl<'r> Quantity<'r> {
    /// `value` written in `unit`, read as a quantity of `kind`.
    pub fn new(value: f64, unit: Unit<'r>, kind: Kind<'r>) -> Result<Self, QuantityError> {
        if !value.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        unit.require_kind(kind)?;
        kind.dimensions()?;
        let (reference, scale, offset) = unit.conversion()?;
        let offset = match kind.role() {
            AffineRole::Point => offset,
            AffineRole::Linear if offset != 0.0 => return Err(unit.offset_on(kind)),
            AffineRole::Linear | AffineRole::Difference => 0.0,
        };
        Self::held(kind, reference, scale * value + offset)
    }
    /// A computed value in `unit`, which must be finite and inside the kind's
    /// declared range. A unit that is not of `kind` (a point's unit holding a
    /// difference of two points) hands the value to the kind's canonical unit.
    fn held(kind: Kind<'r>, unit: Unit<'r>, value: f64) -> Result<Self, QuantityError> {
        let (unit, value) = if unit.kinds().any(|k| k == kind) {
            (unit, value)
        } else {
            let canonical = kind.canonical_unit()?;
            (canonical, value * coherent_ratio(unit, canonical)?)
        };
        if !value.is_finite() {
            return Err(QuantityError::NumericalFailure);
        }
        if kind.minimum().is_some_and(|minimum| value < minimum) {
            return Err(QuantityError::BelowMinimum {
                kind: kind.id().into(),
            });
        }
        Ok(Self { kind, unit, value })
    }
    pub fn kind(self) -> Kind<'r> {
        self.kind
    }
    /// This value expressed in another terminal unit of its kind. A point
    /// kind's references need not share an origin, so they are not crossed.
    fn value_in(self, target: Unit<'r>) -> Result<f64, QuantityError> {
        if self.unit == target {
            return Ok(self.value);
        }
        if self.kind.role() == AffineRole::Point {
            return Err(QuantityError::DisconnectedConversion);
        }
        let value = self.value * coherent_ratio(self.unit, target)?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(QuantityError::NumericalFailure)
        }
    }
    pub fn in_unit(self, unit: Unit<'r>) -> Result<f64, QuantityError> {
        unit.require_kind(self.kind)?;
        let (reference, scale, offset) = unit.conversion()?;
        let offset = match self.kind.role() {
            AffineRole::Point => offset,
            AffineRole::Linear | AffineRole::Difference => 0.0,
        };
        let value = (self.value_in(reference)? - offset) / scale;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(QuantityError::NumericalFailure)
        }
    }
    /// A caller boundary: the value in the unit of this kind with `symbol`.
    pub fn in_symbol(self, symbol: &str) -> Result<f64, QuantityError> {
        let units = self.kind.registry().units_for_symbol(symbol)?;
        let mut candidates = units
            .into_iter()
            .filter(|unit| unit.kinds().any(|k| k == self.kind));
        match (candidates.next(), candidates.next()) {
            (Some(unit), None) => self.in_unit(unit),
            (Some(_), Some(_)) => Err(QuantityError::AmbiguousUnit(symbol.into())),
            (None, _) => Err(QuantityError::UnitKindMismatch {
                unit: symbol.into(),
                kind: self.kind.id().into(),
            }),
        }
    }
    /// Every binary operation ends here; `Kind::combine` decides the result's
    /// kind before any arithmetic is done.
    pub fn apply(self, op: Op, other: Self) -> Result<Self, QuantityError> {
        let kind = self.kind.combine(op, other.kind)?;
        match op.product() {
            None => {
                // A difference joins a point in the point's unit.
                let joins_point = other.kind.role() == AffineRole::Point;
                if joins_point && self.kind.role() != AffineRole::Point {
                    return other.apply(op, self);
                }
                let b = other.value_in(self.unit)?;
                let value = if op == Op::Add {
                    self.value + b
                } else {
                    self.value - b
                };
                Self::held(kind, self.unit, value)
            }
            Some(product) => {
                let unit = kind.canonical_unit()?;
                let (a, b) = (self.value, other.value);
                // A quantity holds one coefficient; the grade lives in the kind.
                let value = match product {
                    ProductOp::Mul | ProductOp::Dot | ProductOp::Wedge => a * b,
                    ProductOp::Div if b == 0.0 => return Err(QuantityError::DivisionByZero),
                    ProductOp::Div => a / b,
                };
                let scale = |u: Unit<'r>| u.coherent_scale().cloned();
                let factor = match product {
                    ProductOp::Mul | ProductOp::Dot | ProductOp::Wedge => {
                        scale(self.unit)?.multiply(&scale(other.unit)?)
                    }
                    ProductOp::Div => scale(self.unit)?
                        .divide(&scale(other.unit)?)
                        .ok_or(QuantityError::DivisionByZero)?,
                }
                .divide(&scale(unit)?)
                .ok_or(QuantityError::DivisionByZero)?
                .to_f64()
                .ok_or(QuantityError::NumericalFailure)?;
                Self::held(kind, unit, value * factor)
            }
        }
    }
    /// The order of two quantities of one kind; different kinds have none.
    pub fn compare(self, other: Self) -> Result<Ordering, QuantityError> {
        self.same_kind(other)?;
        self.value
            .partial_cmp(&other.value_in(self.unit)?)
            .ok_or(QuantityError::NumericalFailure)
    }
    /// The magnitude with its sign dropped; a point has no magnitude.
    pub fn abs(self) -> Result<Self, QuantityError> {
        self.linear(Operation::Abs)?;
        Ok(Self {
            value: self.value.abs(),
            ..self
        })
    }
    pub fn is_zero(self) -> bool {
        self.value == 0.0
    }
    /// Whether `|self| <= tolerance`, for a tolerance of the same kind.
    pub fn within(self, tolerance: Self) -> Result<bool, QuantityError> {
        self.same_kind(tolerance)?;
        Ok(self.abs()?.value <= tolerance.value_in(self.unit)?)
    }
    pub fn scale(self, factor: f64) -> Result<Self, QuantityError> {
        self.linear(Operation::Scale)?;
        if !factor.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        Self::held(self.kind, self.unit, self.value * factor)
    }
    pub fn divide_scalar(self, divisor: f64) -> Result<Self, QuantityError> {
        self.linear(Operation::DivideScalar)?;
        if !divisor.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        if divisor == 0.0 {
            return Err(QuantityError::DivisionByZero);
        }
        Self::held(self.kind, self.unit, self.value / divisor)
    }
    fn linear(self, operation: Operation) -> Result<(), QuantityError> {
        if self.kind.role() == AffineRole::Point {
            return Err(QuantityError::UnsupportedOperation {
                operation,
                left: self.kind.id().into(),
                right: None,
            });
        }
        Ok(())
    }
    fn same_kind(self, other: Self) -> Result<(), QuantityError> {
        if self.kind == other.kind {
            Ok(())
        } else {
            Err(QuantityError::KindMismatch {
                expected: self.kind.id().into(),
                actual: other.kind.id().into(),
            })
        }
    }
}

/// How many of `to` one of `from` is, by the catalog's coherent scales.
fn coherent_ratio(from: Unit<'_>, to: Unit<'_>) -> Result<f64, QuantityError> {
    from.coherent_scale()?
        .divide(to.coherent_scale()?)
        .ok_or(QuantityError::DivisionByZero)?
        .to_f64()
        .ok_or(QuantityError::NumericalFailure)
}
