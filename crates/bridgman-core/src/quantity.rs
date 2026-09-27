//! The one quantity: a finite value of a registry kind, held in a reference
//! unit of that kind. Every operation asks the registry which kind results.
use crate::{
    AffineRole, ExactScalar, ExactValue, Kind, Op, Operation, ProductOp, QuantityError, Unit,
};
use std::cmp::Ordering;
use std::fmt;

/// A quantity has no `==`: its value is a binary64 number in whichever
/// reference unit it is held in, so derived equality would call `1000 g` and
/// `1 kg` different. Compare with [`Quantity::equals_exactly`], which refuses
/// quantities of different kinds.
///
/// ```compile_fail
/// fn same(a: bridgman_core::Quantity<'_>, b: bridgman_core::Quantity<'_>) -> bool {
///     a == b
/// }
/// ```
#[derive(Clone, Copy, Debug)]
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
    pub fn new(value: f64, unit: Unit<'r>, kind: Kind<'r>) -> Result<Self, QuantityError<'r>> {
        if !value.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        unit.require_kind(kind)?;
        kind.dimensions()?;
        let (reference, scale, offset) = unit.conversion()?;
        let offset = unit.offset_for(kind, offset)?;
        Self::held(kind, reference, scale * value + offset)
    }
    /// A computed value in `unit`, which must be finite and inside the kind's
    /// declared range. A unit that is not of `kind` (a point's unit holding a
    /// difference of two points) hands the value to the kind's canonical unit.
    fn held(kind: Kind<'r>, unit: Unit<'r>, value: f64) -> Result<Self, QuantityError<'r>> {
        let (unit, value) = if unit.kinds().any(|k| k == kind) {
            (unit, value)
        } else {
            let canonical = kind.canonical_unit()?;
            (canonical, value * coherent_ratio(unit, canonical)?)
        };
        if !value.is_finite() {
            return Err(QuantityError::NumericalFailure);
        }
        kind.check_minimum(value)?;
        Ok(Self { kind, unit, value })
    }
    /// A value compile has already checked (a declared floor).
    pub(crate) fn declared(kind: Kind<'r>, unit: Unit<'r>, value: f64) -> Self {
        Self { kind, unit, value }
    }
    pub fn kind(self) -> Kind<'r> {
        self.kind
    }
    /// This value expressed in another terminal unit of its kind. A point
    /// kind's references need not share an origin, so they are not crossed.
    fn value_in(self, target: Unit<'r>) -> Result<f64, QuantityError<'r>> {
        if self.unit == target {
            return Ok(self.value);
        }
        self.crosses_to(target)?;
        let value = self.value * coherent_ratio(self.unit, target)?;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(QuantityError::NumericalFailure)
        }
    }
    /// Refuse carrying this value to another terminal unit of its kind by
    /// coherent scales when its kind is a point: a point kind's references
    /// need not share an origin.
    fn crosses_to(self, target: Unit<'r>) -> Result<(), QuantityError<'r>> {
        match self.kind.role() {
            AffineRole::Point => Err(QuantityError::DisconnectedConversion {
                from: self.unit,
                to: target,
            }),
            AffineRole::Linear | AffineRole::Difference => Ok(()),
        }
    }
    pub fn in_unit(self, unit: Unit<'r>) -> Result<f64, QuantityError<'r>> {
        unit.require_kind(self.kind)?;
        let (reference, scale, offset) = unit.conversion()?;
        let offset = unit.offset_for(self.kind, offset)?;
        let value = (self.value_in(reference)? - offset) / scale;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(QuantityError::NumericalFailure)
        }
    }
    /// A caller boundary: the value in the unit of this kind with `symbol`.
    pub fn in_symbol(self, symbol: &str) -> Result<f64, QuantityError<'r>> {
        self.in_unit(self.kind.registry().symbol_unit(symbol, self.kind)?)
    }
    /// Every binary operation ends here; `Kind::combine` decides the result's
    /// kind before any arithmetic is done.
    pub fn apply(self, op: Op, other: Self) -> Result<Self, QuantityError<'r>> {
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
    /// Whether two quantities of one kind are the same value, compared
    /// exactly: each held binary64 value is read as the rational it is, and
    /// `other` is carried into `self`'s unit by the catalog's exact coherent
    /// scales. Quantities of different kinds are refused, not unequal.
    pub fn equals_exactly(self, other: Self) -> Result<bool, QuantityError<'r>> {
        self.same_kind(other)?;
        let exact = |value: f64| {
            ExactScalar::from_f64(value)
                .map(ExactValue::from_scalar)
                .ok_or(QuantityError::NumericalFailure)
        };
        let carried = if other.unit == self.unit {
            exact(other.value)?
        } else {
            other.crosses_to(self.unit)?;
            let ratio = other
                .unit
                .coherent_scale()?
                .divide(self.unit.coherent_scale()?)
                .ok_or(QuantityError::DivisionByZero)?;
            exact(other.value)?.multiply_scalar(&ratio)
        };
        Ok(exact(self.value)? == carried)
    }
    /// The order of two quantities of one kind; different kinds have none.
    pub fn compare(self, other: Self) -> Result<Ordering, QuantityError<'r>> {
        self.same_kind(other)?;
        self.value
            .partial_cmp(&other.value_in(self.unit)?)
            .ok_or(QuantityError::NumericalFailure)
    }
    /// The magnitude with its sign dropped; a point has no magnitude.
    pub fn abs(self) -> Result<Self, QuantityError<'r>> {
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
    pub fn within(self, tolerance: Self) -> Result<bool, QuantityError<'r>> {
        self.same_kind(tolerance)?;
        Ok(self.abs()?.value <= tolerance.value_in(self.unit)?)
    }
    pub fn scale(self, factor: f64) -> Result<Self, QuantityError<'r>> {
        self.linear(Operation::Scale)?;
        if !factor.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        Self::held(self.kind, self.unit, self.value * factor)
    }
    pub fn divide_scalar(self, divisor: f64) -> Result<Self, QuantityError<'r>> {
        self.linear(Operation::DivideScalar)?;
        if !divisor.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        if divisor == 0.0 {
            return Err(QuantityError::DivisionByZero);
        }
        Self::held(self.kind, self.unit, self.value / divisor)
    }
    fn linear(self, operation: Operation) -> Result<(), QuantityError<'r>> {
        if self.kind.role() == AffineRole::Point {
            return Err(self.kind.refuse(operation, None));
        }
        Ok(())
    }
    fn same_kind(self, other: Self) -> Result<(), QuantityError<'r>> {
        if self.kind == other.kind {
            Ok(())
        } else {
            Err(self.kind.mismatch(other.kind))
        }
    }
}

/// How many of `to` one of `from` is, by the catalog's coherent scales.
fn coherent_ratio<'r>(from: Unit<'r>, to: Unit<'r>) -> Result<f64, QuantityError<'r>> {
    from.coherent_scale()?
        .divide(to.coherent_scale()?)
        .ok_or(QuantityError::DivisionByZero)?
        .to_f64()
        .ok_or(QuantityError::NumericalFailure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    const MASSES: &str = r#"
schema: 4
kinds:
  - {id: mass, dimensions: {M: 1}}
  - {id: energy, dimensions: {M: 1, L: 2, T: -2}}
  - {id: torque, dimensions: {M: 1, L: 2, T: -2}}
units:
  - {id: gram, symbol: g, kinds: [mass], conversion: {reference_unit: gram, scale: '1'}, coherent_scale: '1/1000'}
  - {id: kilogram, symbol: kg, kinds: [mass], conversion: {reference_unit: gram, scale: '1000'}, coherent_scale: '1'}
  - {id: tonne, symbol: t, kinds: [mass], conversion: {reference_unit: tonne, scale: '1'}, coherent_scale: '1000'}
  - {id: joule, symbol: J, kinds: [energy], conversion: {reference_unit: joule, scale: '1'}, coherent_scale: '1'}
  - {id: newton_metre, symbol: N*m, kinds: [torque], conversion: {reference_unit: newton_metre, scale: '1'}, coherent_scale: '1'}
"#;

    #[test]
    fn quantities_have_no_equality() {
        // An inherent constant bound on `PartialEq` shadows the trait's
        // fallback only for types that implement it.
        struct Probe<T>(std::marker::PhantomData<T>);
        trait Fallback {
            const EQ: bool = false;
        }
        impl<T> Fallback for Probe<T> {}
        #[allow(dead_code)]
        impl<T: PartialEq> Probe<T> {
            const EQ: bool = true;
        }
        assert!(Probe::<f64>::EQ);
        assert!(!Probe::<Quantity<'static>>::EQ);
    }
    #[test]
    fn equal_values_in_different_units_are_exactly_equal() {
        let r = Registry::from_yaml(MASSES).unwrap();
        let q = |value, symbol| r.quantity_for_symbol(value, symbol, None).unwrap();
        assert_eq!(q(1000.0, "g").equals_exactly(q(1.0, "kg")), Ok(true));
        assert_eq!(q(500000.0, "g").equals_exactly(q(0.5, "t")), Ok(true));
        assert_eq!(q(0.5, "t").equals_exactly(q(500000.0, "g")), Ok(true));
        assert_eq!(q(2.0, "kg").equals_exactly(q(1.0, "kg")), Ok(false));
        // 0.1 t is not a binary64 number, so it is not exactly 100000 g.
        assert_eq!(q(0.1, "t").equals_exactly(q(100000.0, "g")), Ok(false));
        let (energy, torque) = (r.kind("energy").unwrap(), r.kind("torque").unwrap());
        assert_eq!(
            q(1.0, "J").equals_exactly(q(1.0, "N*m")),
            Err(QuantityError::KindMismatch {
                expected: energy,
                actual: torque,
            })
        );
    }
}
