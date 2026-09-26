//! One quantity: homogeneous G3 coordinates of a registry kind in a reference unit.
use crate::{AffineRole, Grade, Kind, Op, Operation, ProductOp, QuantityError, Unit};
use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity<'r> {
    kind: Kind<'r>,
    unit: Unit<'r>,
    // A homogeneous G3 grade has at most three components. Unused slots are zero.
    values: [f64; 3],
}

impl fmt::Display for Quantity<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.kind.grade().blades().count() == 1 {
            self.values[0].fmt(f)?;
        } else {
            write!(f, "{:?}", self.values)?;
        }
        if f.alternate() {
            write!(f, "[{}]", self.unit.symbol())
        } else {
            write!(f, " {}", self.unit.symbol())
        }
    }
}

impl<'r> Quantity<'r> {
    /// A one-component value. Vector and bivector kinds require `from_components`.
    pub fn new(value: f64, unit: Unit<'r>, kind: Kind<'r>) -> Result<Self, QuantityError> {
        Self::from_components(&[value], unit, kind)
    }
    /// Coordinates in `Grade::blades` order: x/y/z or xy/xz/yz.
    /// Frames belong to consumer morphisms, not to this value.
    pub fn from_components(
        values: &[f64],
        unit: Unit<'r>,
        kind: Kind<'r>,
    ) -> Result<Self, QuantityError> {
        let expected = kind.grade().blades().count();
        if values.len() != expected {
            return Err(QuantityError::Components {
                kind: kind.id().into(),
                grade: kind.grade(),
                expected,
                actual: values.len(),
            });
        }
        if values.iter().any(|value| !value.is_finite()) {
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
        let mut held = [0.0; 3];
        for (target, value) in held.iter_mut().zip(values) {
            *target = scale * value + offset;
        }
        Self::held(kind, reference, held)
    }
    fn held(kind: Kind<'r>, unit: Unit<'r>, mut values: [f64; 3]) -> Result<Self, QuantityError> {
        let unit = if unit.kinds().any(|k| k == kind) {
            unit
        } else {
            let canonical = kind.canonical_unit()?;
            let ratio = coherent_ratio(unit, canonical)?;
            values = values.map(|value| value * ratio);
            canonical
        };
        if values.iter().any(|value| !value.is_finite()) {
            return Err(QuantityError::NumericalFailure);
        }
        if kind.grade() == Grade::Scalar {
            kind.check_minimum(values[0])?;
        }
        Ok(Self { kind, unit, values })
    }
    pub(crate) fn declared(kind: Kind<'r>, unit: Unit<'r>, value: f64) -> Self {
        Self {
            kind,
            unit,
            values: [value, 0.0, 0.0],
        }
    }
    pub fn kind(self) -> Kind<'r> {
        self.kind
    }
    fn values_in(self, target: Unit<'r>) -> Result<[f64; 3], QuantityError> {
        if self.unit == target {
            return Ok(self.values);
        }
        if self.kind.role() == AffineRole::Point {
            return Err(QuantityError::DisconnectedConversion);
        }
        let ratio = coherent_ratio(self.unit, target)?;
        let values = self.values.map(|value| value * ratio);
        if values.iter().all(|value| value.is_finite()) {
            Ok(values)
        } else {
            Err(QuantityError::NumericalFailure)
        }
    }
    fn converted(self, unit: Unit<'r>) -> Result<[f64; 3], QuantityError> {
        unit.require_kind(self.kind)?;
        let (reference, scale, offset) = unit.conversion()?;
        let offset = match self.kind.role() {
            AffineRole::Point => offset,
            AffineRole::Linear | AffineRole::Difference => 0.0,
        };
        let mut values = self.values_in(reference)?;
        for value in values.iter_mut().take(self.kind.grade().blades().count()) {
            *value = (*value - offset) / scale;
        }
        if values.iter().all(|value| value.is_finite()) {
            Ok(values)
        } else {
            Err(QuantityError::NumericalFailure)
        }
    }
    /// A caller-boundary reading of every component in the requested unit.
    pub fn components_in(self, unit: Unit<'r>) -> Result<Vec<f64>, QuantityError> {
        Ok(self.converted(unit)?[..self.kind.grade().blades().count()].to_vec())
    }
    pub fn in_unit(self, unit: Unit<'r>) -> Result<f64, QuantityError> {
        self.scalar(Operation::ReadScalar)?;
        Ok(self.converted(unit)?[0])
    }
    /// A caller boundary; a vector cannot be silently projected onto one axis.
    pub fn in_symbol(self, symbol: &str) -> Result<f64, QuantityError> {
        self.in_unit(self.symbol_unit(symbol)?)
    }
    pub fn components_in_symbol(self, symbol: &str) -> Result<Vec<f64>, QuantityError> {
        self.components_in(self.symbol_unit(symbol)?)
    }
    fn symbol_unit(self, symbol: &str) -> Result<Unit<'r>, QuantityError> {
        let units = self.kind.registry().units_for_symbol(symbol)?;
        let mut candidates = units
            .into_iter()
            .filter(|unit| unit.kinds().any(|k| k == self.kind));
        match (candidates.next(), candidates.next()) {
            (Some(unit), None) => Ok(unit),
            (Some(_), Some(_)) => Err(QuantityError::AmbiguousUnit(symbol.into())),
            (None, _) => Err(QuantityError::UnitKindMismatch {
                unit: symbol.into(),
                kind: self.kind.id().into(),
            }),
        }
    }
    /// Kind algebra chooses the result grade; arithmetic projects the G3 product
    /// onto that grade. Dot uses the grade-|a-b| geometric product convention.
    pub fn apply(self, op: Op, other: Self) -> Result<Self, QuantityError> {
        let kind = self.kind.combine(op, other.kind)?;
        match op.product() {
            None => {
                if other.kind.role() == AffineRole::Point && self.kind.role() != AffineRole::Point {
                    return other.apply(op, self);
                }
                let other = other.values_in(self.unit)?;
                let values = std::array::from_fn(|i| {
                    if op == Op::Add {
                        self.values[i] + other[i]
                    } else {
                        self.values[i] - other[i]
                    }
                });
                Self::held(kind, self.unit, values)
            }
            Some(product) => {
                let unit = kind.canonical_unit()?;
                let mut values = [0.0; 3];
                match product {
                    ProductOp::Div => {
                        if other.values[0] == 0.0 {
                            return Err(QuantityError::DivisionByZero);
                        }
                        values = self.values.map(|value| value / other.values[0]);
                    }
                    ProductOp::Mul | ProductOp::Dot | ProductOp::Wedge => {
                        for (a, blade_a) in self.kind.grade().blades().enumerate() {
                            for (b, blade_b) in other.kind.grade().blades().enumerate() {
                                let result = blade_a ^ blade_b;
                                if let Some(slot) =
                                    kind.grade().blades().position(|blade| blade == result)
                                {
                                    values[slot] += blade_sign(blade_a, blade_b)
                                        * self.values[a]
                                        * other.values[b];
                                }
                            }
                        }
                    }
                }
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
                Self::held(kind, unit, values.map(|value| value * factor))
            }
        }
    }
    /// Apply a linear map's induced exterior power to this grade. The caller
    /// owns the chart and any isometry/affine obligations; no frame is stored.
    pub fn map_linear(self, matrix: [[f64; 3]; 3]) -> Result<Self, QuantityError> {
        if matrix.iter().flatten().any(|entry| !entry.is_finite()) {
            return Err(QuantityError::NonFiniteInput);
        }
        let mut values = [0.0; 3];
        for (index, input_blade) in self.kind.grade().blades().enumerate() {
            let mut image = [0.0; 8];
            image[0] = self.values[index];
            for column in (0..3).filter(|column| input_blade & (1 << column) != 0) {
                let mut next = [0.0; 8];
                for (blade, coefficient) in image.into_iter().enumerate() {
                    for (row, entries) in matrix.iter().enumerate() {
                        let axis = 1 << row;
                        if blade & axis == 0 {
                            next[blade | axis] +=
                                coefficient * entries[column] * blade_sign(blade as u8, axis as u8);
                        }
                    }
                }
                image = next;
            }
            for (index, output_blade) in self.kind.grade().blades().enumerate() {
                values[index] += image[usize::from(output_blade)];
            }
        }
        Self::held(self.kind, self.unit, values)
    }
    pub fn compare(self, other: Self) -> Result<Ordering, QuantityError> {
        self.same_kind(other)?;
        self.scalar(Operation::Compare)?;
        self.values[0]
            .partial_cmp(&other.values_in(self.unit)?[0])
            .ok_or(QuantityError::NumericalFailure)
    }
    pub fn abs(self) -> Result<Self, QuantityError> {
        self.linear(Operation::Abs)?;
        self.scalar(Operation::Abs)?;
        Ok(Self {
            values: [self.values[0].abs(), 0.0, 0.0],
            ..self
        })
    }
    pub fn is_zero(self) -> bool {
        self.values.iter().all(|value| *value == 0.0)
    }
    /// Scalar absolute bounds or Euclidean coefficient-norm bounds for higher
    /// grades. A bound's direction is irrelevant; no componentwise box is used.
    pub fn within(self, tolerance: Self) -> Result<bool, QuantityError> {
        self.same_kind(tolerance)?;
        self.linear(Operation::Abs)?;
        let bound = tolerance.values_in(self.unit)?;
        if self.kind.grade() == Grade::Scalar {
            return Ok(self.values[0].abs() <= bound[0]);
        }
        let scale = self
            .values
            .iter()
            .chain(&bound)
            .fold(0.0_f64, |a, b| a.max(b.abs()));
        if scale == 0.0 {
            return Ok(true);
        }
        let norm = |values: [f64; 3]| {
            values
                .into_iter()
                .map(|value| value / scale)
                .fold(0.0_f64, f64::hypot)
        };
        Ok(norm(self.values) <= norm(bound))
    }
    pub fn scale(self, factor: f64) -> Result<Self, QuantityError> {
        self.linear(Operation::Scale)?;
        if !factor.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        Self::held(
            self.kind,
            self.unit,
            self.values.map(|value| value * factor),
        )
    }
    pub fn divide_scalar(self, divisor: f64) -> Result<Self, QuantityError> {
        self.linear(Operation::DivideScalar)?;
        if !divisor.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        if divisor == 0.0 {
            return Err(QuantityError::DivisionByZero);
        }
        Self::held(
            self.kind,
            self.unit,
            self.values.map(|value| value / divisor),
        )
    }
    fn refused(self, operation: Operation) -> QuantityError {
        QuantityError::UnsupportedOperation {
            operation,
            left: self.kind.id().into(),
            right: None,
        }
    }
    fn scalar(self, operation: Operation) -> Result<(), QuantityError> {
        if self.kind.grade() != Grade::Scalar {
            Err(self.refused(operation))
        } else {
            Ok(())
        }
    }
    fn linear(self, operation: Operation) -> Result<(), QuantityError> {
        if self.kind.role() == AffineRole::Point {
            Err(self.refused(operation))
        } else {
            Ok(())
        }
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

fn blade_sign(left: u8, right: u8) -> f64 {
    let swaps: u32 = (0..3)
        .filter(|bit| left & (1 << bit) != 0)
        .map(|bit| (right & ((1 << bit) - 1)).count_ones())
        .sum();
    if swaps % 2 == 0 {
        1.0
    } else {
        -1.0
    }
}

fn coherent_ratio(from: Unit<'_>, to: Unit<'_>) -> Result<f64, QuantityError> {
    from.coherent_scale()?
        .divide(to.coherent_scale()?)
        .ok_or(QuantityError::DivisionByZero)?
        .to_f64()
        .ok_or(QuantityError::NumericalFailure)
}
