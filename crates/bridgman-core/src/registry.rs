//! A compiled catalog. Kinds and units become handles that carry their
//! registry, so every judgement about them is made here, once: which kinds
//! combine and how, which unit converts to which, and where a kind's values end.
use crate::catalog::{Magnitude, Op, ProductOp};
use crate::derive::{candidates, operand, resolve, Operand, Resolved};
use crate::{
    DerivationError, Dimensions, ExactScalar, ExactValue, Grade, Operation, Quantity,
    QuantityError, Record,
};
use num_rational::BigRational;
use num_traits::{One, Zero};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::{Hash, Hasher};

/// How a kind takes part in additive arithmetic. It follows from the declared
/// affine spaces: a kind naming a `difference_kind` is a point kind, a kind
/// named as one is a difference kind, and every other kind is linear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AffineRole {
    Linear,
    Point,
    Difference,
}
impl AffineRole {
    /// The part of a unit's conversion offset a value of this role takes. An
    /// offset moves an origin, so it applies to points only: a difference of
    /// points takes none of it, and a linear kind has no origin to move, so a
    /// nonzero offset is refused (`None`) rather than dropped.
    pub fn offset<T: Default + PartialEq>(self, offset: T) -> Option<T> {
        match self {
            Self::Point => Some(offset),
            Self::Difference => Some(T::default()),
            Self::Linear => (offset == T::default()).then_some(offset),
        }
    }
}

/// A kind as `compile` resolved it.
#[derive(Clone, Debug)]
pub(crate) struct CompiledKind {
    pub(crate) id: String,
    pub(crate) dimensions: Option<Dimensions>,
    pub(crate) grade: Grade,
    pub(crate) role: AffineRole,
    /// A point kind's declared difference kind.
    pub(crate) difference: Option<usize>,
    /// The first terminal unit declared for the kind.
    pub(crate) canonical: Option<usize>,
    /// The least value, in the canonical unit.
    pub(crate) minimum: Option<Minimum>,
    /// Declared: this kind × duration is `rate_of`.
    pub(crate) rate_of: Option<usize>,
    /// Derived by compile: the one kind whose `rate_of` is this kind.
    pub(crate) rate: Option<usize>,
}

/// A kind's declared least value.
#[derive(Clone, Debug)]
pub(crate) struct Minimum {
    /// As declared, in the canonical unit.
    pub(crate) declared: ExactScalar,
    /// `declared` as binary64, checked finite by compile.
    pub(crate) value: f64,
    /// The canonical unit, which every unit of the kind reaches.
    pub(crate) unit: usize,
}

/// A unit as `compile` resolved it: every kind and reference it names is an
/// index, checked once, so nothing looks up an id again.
#[derive(Clone, Debug)]
pub(crate) struct CompiledUnit {
    pub(crate) id: String,
    pub(crate) symbol: String,
    pub(crate) kinds: Vec<usize>,
    /// Absent while the source leaves the unit's conversion unresolved.
    pub(crate) conversion: Option<CompiledConversion>,
    pub(crate) coherent_scale: Option<ExactScalar>,
}

/// A declared `Conversion` whose reference unit is resolved.
#[derive(Clone, Debug)]
pub(crate) struct CompiledConversion {
    pub(crate) reference: usize,
    pub(crate) scale: Magnitude<ExactScalar>,
    pub(crate) offset: Magnitude<ExactValue>,
}

/// A declared row that chooses between twins.
#[derive(Clone, Debug)]
pub(crate) struct TwinRow {
    pub(crate) result: usize,
    pub(crate) provenance: Option<String>,
}

/// A compiled catalog; `compile` is the only way to obtain one.
#[derive(Clone, Debug)]
pub struct Registry {
    pub(crate) kinds: Vec<CompiledKind>,
    pub(crate) units: Vec<CompiledUnit>,
    pub(crate) kind_ids: HashMap<String, usize>,
    pub(crate) unit_ids: HashMap<String, usize>,
    pub(crate) symbols: HashMap<String, Vec<usize>>,
    pub(crate) twins: HashMap<(usize, ProductOp, usize), TwinRow>,
    pub(crate) dimensionless: Option<usize>,
    pub(crate) time: Option<usize>,
    pub(crate) provenance: BTreeMap<String, String>,
}

/// A kind of a registry. Handles of different registries are never equal.
#[derive(Clone, Copy)]
pub struct Kind<'r> {
    registry: &'r Registry,
    index: usize,
}
/// A unit of a registry.
#[derive(Clone, Copy)]
pub struct Unit<'r> {
    registry: &'r Registry,
    index: usize,
}

/// Identity, order and display of a handle: its registry and its position,
/// shown (and serialized, as in an error report) by its declared id.
macro_rules! handle {
    ($handle:ident) => {
        impl PartialEq for $handle<'_> {
            fn eq(&self, other: &Self) -> bool {
                std::ptr::eq(self.registry, other.registry) && self.index == other.index
            }
        }
        impl Eq for $handle<'_> {}
        impl Hash for $handle<'_> {
            fn hash<H: Hasher>(&self, state: &mut H) {
                std::ptr::hash(self.registry, state);
                self.index.hash(state);
            }
        }
        impl Ord for $handle<'_> {
            fn cmp(&self, other: &Self) -> Ordering {
                let registry = |h: &Self| h.registry as *const Registry as usize;
                (registry(self), self.index).cmp(&(registry(other), other.index))
            }
        }
        impl PartialOrd for $handle<'_> {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }
        impl fmt::Debug for $handle<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($handle))
                    .field(&self.id())
                    .finish()
            }
        }
        impl fmt::Display for $handle<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.id())
            }
        }
        impl serde::Serialize for $handle<'_> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.id())
            }
        }
    };
}
handle!(Kind);
handle!(Unit);

impl<'r> Kind<'r> {
    fn compiled(self) -> &'r CompiledKind {
        &self.registry.kinds[self.index]
    }
    fn at(self, index: usize) -> Self {
        Self { index, ..self }
    }
    pub fn registry(self) -> &'r Registry {
        self.registry
    }
    pub fn id(self) -> &'r str {
        &self.compiled().id
    }
    pub fn role(self) -> AffineRole {
        self.compiled().role
    }
    pub fn grade(self) -> Grade {
        self.compiled().grade
    }
    /// The kind this one is the rate of, if it is a rate.
    pub fn rate_of(self) -> Option<Kind<'r>> {
        self.compiled().rate_of.map(|i| self.at(i))
    }
    /// The rate of this kind, if a kind declares itself so.
    pub fn rate(self) -> Option<Kind<'r>> {
        self.compiled().rate.map(|i| self.at(i))
    }
    pub fn dimensions(self) -> Result<&'r Dimensions, QuantityError<'r>> {
        self.compiled().dimensions.as_ref().ok_or_else(|| {
            QuantityError::Derivation(DerivationError::UnresolvedDimensions { kind: self })
        })
    }
    /// The unit a computed quantity of this kind is held in.
    pub fn canonical_unit(self) -> Result<Unit<'r>, QuantityError<'r>> {
        let index = self
            .compiled()
            .canonical
            .ok_or(QuantityError::NoCanonicalUnit { kind: self })?;
        Ok(Unit {
            registry: self.registry,
            index,
        })
    }
    /// The least value a quantity of this kind may take, in its canonical unit,
    /// or `None` when the kind declares no floor.
    pub fn minimum(self) -> Option<Quantity<'r>> {
        self.compiled().minimum.as_ref().map(|m| {
            Quantity::declared(
                self,
                Unit {
                    registry: self.registry,
                    index: m.unit,
                },
                m.value,
            )
        })
    }
    /// Refuse `value`, finite and in the kind's canonical unit, when it lies
    /// below the declared floor.
    pub(crate) fn check_minimum(self, value: f64) -> Result<(), QuantityError<'r>> {
        match &self.compiled().minimum {
            Some(m) if value < m.value => Err(QuantityError::BelowMinimum {
                kind: self,
                unit: Unit {
                    registry: self.registry,
                    index: m.unit,
                },
                minimum: m.declared.clone(),
                value: ExactScalar::from_f64(value).ok_or(QuantityError::NumericalFailure)?,
            }),
            Some(_) | None => Ok(()),
        }
    }
    pub(crate) fn same_registry(self, other: Kind<'_>) -> Result<(), QuantityError<'r>> {
        if std::ptr::eq(self.registry, other.registry) {
            Ok(())
        } else {
            Err(QuantityError::RegistryMismatch)
        }
    }
    /// The kind of `self op other`. This is the only statement of kind
    /// arithmetic: quantities and authored expressions both ask it.
    pub fn combine(self, op: Op, other: Self) -> Result<Self, QuantityError<'r>> {
        self.same_registry(other)?;
        match op.product() {
            Some(product) => self.product(product, other),
            None => self.sum(op, other),
        }
    }
    pub(crate) fn refuse(self, operation: Operation, other: Option<Self>) -> QuantityError<'r> {
        QuantityError::UnsupportedOperation {
            operation,
            left: self,
            right: other,
        }
    }
    pub(crate) fn mismatch(self, other: Self) -> QuantityError<'r> {
        QuantityError::KindMismatch {
            expected: self,
            actual: other,
        }
    }
    /// The kind of a difference of two values of this kind: a point kind's
    /// declared difference kind, and any other kind itself.
    pub fn difference(self) -> Self {
        self.compiled()
            .difference
            .map_or(self, |index| self.at(index))
    }
    /// What a product reads of this kind: its dimensions, grade and role.
    pub fn operand(self) -> Result<Operand, QuantityError<'r>> {
        operand(&self.registry.kinds, self.index)
            .map_err(|error| error.map_kinds(|index| self.at(index)).into())
    }
    /// Addition and subtraction: one kind with itself, and a point kind with
    /// its declared difference kind. Two points differ; they never add.
    fn sum(self, op: Op, other: Self) -> Result<Self, QuantityError<'r>> {
        let refuse = || self.refuse(Operation::Binary(op), Some(other));
        match (self.role(), other.role()) {
            (AffineRole::Point, AffineRole::Point) if op == Op::Sub => {
                if self != other {
                    return Err(self.mismatch(other));
                }
                Ok(self.difference())
            }
            (AffineRole::Point, AffineRole::Point) => Err(refuse()),
            (AffineRole::Point, _) => {
                if other != self.difference() {
                    return Err(self.difference().mismatch(other));
                }
                Ok(self)
            }
            (_, AffineRole::Point) if op == Op::Add => other.sum(op, self),
            (_, AffineRole::Point) => Err(refuse()),
            (AffineRole::Linear | AffineRole::Difference, _) => {
                if self != other {
                    return Err(self.mismatch(other));
                }
                Ok(self)
            }
        }
    }
    /// Products and quotients: derivation, and for true twins the declared row.
    pub fn product(self, op: ProductOp, other: Self) -> Result<Self, QuantityError<'r>> {
        self.same_registry(other)?;
        let registry = self.registry;
        let derivation = resolve(
            &registry.kinds,
            registry.dimensionless,
            registry.duration(),
            self.index,
            op,
            other.index,
        )
        .map_err(|error| error.map_kinds(|index| self.at(index)))?;
        match derivation.resolved {
            Resolved::Kind(index) => Ok(self.at(index)),
            Resolved::Twins(twins) => registry
                .twins
                .get(&(self.index, op, other.index))
                .map(|row| self.at(row.result))
                .ok_or_else(|| QuantityError::UnresolvedTwin {
                    left: self,
                    op,
                    right: other,
                    twins: twins.iter().map(|&i| self.at(i)).collect(),
                }),
            Resolved::None => Err(QuantityError::NoProductKind {
                left: self,
                op,
                right: other,
                dimensions: derivation.dimensions,
                grade: derivation.grade,
            }),
        }
    }
    /// The provenance the declared row choosing `self op other` states, if a row
    /// chooses it and states one.
    pub fn row_provenance(
        self,
        op: ProductOp,
        other: Self,
    ) -> Result<Option<&'r str>, QuantityError<'r>> {
        self.same_registry(other)?;
        Ok(self
            .registry
            .twins
            .get(&(self.index, op, other.index))
            .and_then(|row| row.provenance.as_deref()))
    }
    /// The kind of `self` raised to a rational power (a root is a fractional
    /// one), derived from dimensions and grade. The cases are tried in this
    /// order, and the first that applies decides:
    ///
    /// 1. A point kind is refused as `UnsupportedOperation` at every exponent,
    ///    including 1.
    /// 2. The first power is `self`.
    /// 3. Any other power of a graded (non-scalar) kind, including the zeroth, is
    ///    refused as `UngradedPower`.
    /// 4. A kind declared without dimensions is refused as `UnresolvedDimensions`.
    /// 5. When the registry declares a dimensionless kind, every power of that
    ///    kind, and the zeroth power of any other dimensioned scalar kind, is that
    ///    kind.
    /// 6. Otherwise, including a zeroth power when no dimensionless kind is
    ///    declared, the result is the one non-point scalar kind with the power's
    ///    dimensions. If there is none, the power is refused as `NoPowerKind`; if
    ///    there are several, as `UnresolvedPowerTwin`, since rows choose products,
    ///    not powers.
    pub fn power(self, exponent: &BigRational) -> Result<Self, QuantityError<'r>> {
        if self.role() == AffineRole::Point {
            return Err(self.refuse(Operation::Power(exponent.clone()), None));
        }
        if exponent.is_one() {
            return Ok(self);
        }
        let base = self;
        let grade = self
            .grade()
            .power(exponent)
            .ok_or_else(|| QuantityError::UngradedPower {
                base,
                exponent: exponent.clone(),
                grade: self.grade(),
            })?;
        let dimensions = self.dimensions()?.pow(exponent);
        let registry = self.registry;
        if let Some(one) = registry.dimensionless {
            if self.index == one || exponent.is_zero() {
                return Ok(self.at(one));
            }
        }
        match candidates(&registry.kinds, &dimensions, grade).as_slice() {
            [] => Err(QuantityError::NoPowerKind {
                base,
                exponent: exponent.clone(),
                dimensions,
                grade,
            }),
            [index] => Ok(self.at(*index)),
            twins @ [_, _, ..] => Err(QuantityError::UnresolvedPowerTwin {
                base,
                exponent: exponent.clone(),
                twins: twins.iter().map(|&i| self.at(i)).collect(),
            }),
        }
    }
    /// The kind of a value of this kind scaled by a pure number
    /// (`Operation::Scale`, `Operation::DivideScalar`) or stripped of its sign
    /// (`Operation::Abs`): itself, unless it is a point, which has no
    /// magnitude to scale.
    pub fn scaled(self, operation: Operation) -> Result<Self, QuantityError<'r>> {
        match self.role() {
            AffineRole::Point => Err(self.refuse(operation, None)),
            AffineRole::Linear | AffineRole::Difference => Ok(self),
        }
    }
    /// The kind two values share when they are compared, ordered or held
    /// against a tolerance: the one kind, or a `KindMismatch`.
    pub fn same(self, other: Self) -> Result<Self, QuantityError<'r>> {
        self.same_registry(other)?;
        if self == other {
            Ok(self)
        } else {
            Err(self.mismatch(other))
        }
    }
    /// Exact conversion of a value of this kind between two of its units.
    pub fn convert_exact(
        self,
        value: ExactValue,
        from: Unit<'r>,
        to: Unit<'r>,
    ) -> Result<ExactValue, QuantityError<'r>> {
        from.require_kind(self)?;
        to.require_kind(self)?;
        self.dimensions()?;
        let (from_ref, from_scale, from_offset) = from.exact_conversion()?;
        let (to_ref, to_scale, to_offset) = to.exact_conversion()?;
        let from_offset = from.offset_for(self, from_offset)?;
        let to_offset = to.offset_for(self, to_offset)?;
        if from_ref != to_ref {
            return Err(QuantityError::DisconnectedConversion { from, to });
        }
        value
            .multiply_scalar(&from_scale)
            .add(&from_offset)
            .sub(&to_offset)
            .divide_scalar(&to_scale)
    }
}

impl<'r> Unit<'r> {
    fn compiled(self) -> &'r CompiledUnit {
        &self.registry.units[self.index]
    }
    fn at(self, index: usize) -> Self {
        Self { index, ..self }
    }
    pub fn id(self) -> &'r str {
        &self.compiled().id
    }
    pub fn symbol(self) -> &'r str {
        &self.compiled().symbol
    }
    /// The kinds this unit is declared for.
    pub fn kinds(self) -> impl Iterator<Item = Kind<'r>> {
        let registry = self.registry;
        self.compiled()
            .kinds
            .iter()
            .map(move |&index| Kind { registry, index })
    }
    pub(crate) fn require_kind(self, kind: Kind<'r>) -> Result<(), QuantityError<'r>> {
        if !std::ptr::eq(self.registry, kind.registry) {
            return Err(QuantityError::RegistryMismatch);
        }
        if self.kinds().any(|k| k == kind) {
            Ok(())
        } else {
            Err(QuantityError::UnitKindMismatch { unit: self, kind })
        }
    }
    /// The part of this unit's conversion `offset` a value of `kind` takes,
    /// by the kind's affine role.
    pub(crate) fn offset_for<T: Default + PartialEq>(
        self,
        kind: Kind<'r>,
        offset: T,
    ) -> Result<T, QuantityError<'r>> {
        kind.role()
            .offset(offset)
            .ok_or(QuantityError::OffsetOnLinearKind { unit: self, kind })
    }
    /// A quantity of this unit's one kind.
    pub fn quantity(self, value: f64) -> Result<Quantity<'r>, QuantityError<'r>> {
        let kinds: Vec<_> = self.kinds().collect();
        match kinds.as_slice() {
            [kind] => Quantity::new(value, self, *kind),
            [] | [_, _, ..] => Err(QuantityError::AmbiguousKind {
                symbol: self.symbol().into(),
                kinds,
            }),
        }
    }
    pub(crate) fn coherent_scale(self) -> Result<&'r ExactScalar, QuantityError<'r>> {
        self.compiled()
            .coherent_scale
            .as_ref()
            .ok_or(QuantityError::MissingCoherentScale { unit: self })
    }
    fn declared_conversion(self) -> Result<&'r CompiledConversion, QuantityError<'r>> {
        self.compiled()
            .conversion
            .as_ref()
            .ok_or(QuantityError::UnresolvedConversion { unit: self })
    }
    /// The reference unit with `value_in_reference = scale * value + offset`.
    pub(crate) fn conversion(self) -> Result<(Unit<'r>, f64, f64), QuantityError<'r>> {
        let conversion = self.declared_conversion()?;
        let scale = match &conversion.scale {
            Magnitude::Exact(value) => value.to_f64().ok_or(QuantityError::NumericalFailure)?,
            Magnitude::Approximate(value) => *value,
        };
        let offset = match &conversion.offset {
            Magnitude::Exact(value) => value.to_f64()?,
            Magnitude::Approximate(value) => *value,
        };
        if !scale.is_finite() || scale == 0.0 || !offset.is_finite() {
            return Err(QuantityError::NumericalFailure);
        }
        Ok((self.at(conversion.reference), scale, offset))
    }
    fn exact_conversion(self) -> Result<(Unit<'r>, ExactScalar, ExactValue), QuantityError<'r>> {
        let conversion = self.declared_conversion()?;
        let (Magnitude::Exact(scale), Magnitude::Exact(offset)) =
            (&conversion.scale, &conversion.offset)
        else {
            return Err(QuantityError::ApproximateConversion { unit: self });
        };
        Ok((self.at(conversion.reference), scale.clone(), offset.clone()))
    }
}

impl Registry {
    pub fn provenance(&self) -> &BTreeMap<String, String> {
        &self.provenance
    }
    /// The point kind of instants, if the catalog declares one.
    pub fn time(&self) -> Option<Kind<'_>> {
        self.time.map(|index| Kind {
            registry: self,
            index,
        })
    }
    pub(crate) fn duration(&self) -> Option<usize> {
        self.time.and_then(|t| self.kinds[t].difference)
    }
    pub fn kind(&self, id: &str) -> Result<Kind<'_>, QuantityError<'_>> {
        let index = *self
            .kind_ids
            .get(id)
            .ok_or_else(|| unknown(Record::Kind, id))?;
        Ok(Kind {
            registry: self,
            index,
        })
    }
    /// Every declared kind, in declaration order.
    pub fn kinds(&self) -> impl ExactSizeIterator<Item = Kind<'_>> {
        (0..self.kinds.len()).map(move |index| Kind {
            registry: self,
            index,
        })
    }
    pub fn unit(&self, id: &str) -> Result<Unit<'_>, QuantityError<'_>> {
        let index = *self
            .unit_ids
            .get(id)
            .ok_or_else(|| unknown(Record::Unit, id))?;
        Ok(Unit {
            registry: self,
            index,
        })
    }
    /// Every declared unit, in declaration order.
    pub fn units(&self) -> impl ExactSizeIterator<Item = Unit<'_>> {
        (0..self.units.len()).map(move |index| Unit {
            registry: self,
            index,
        })
    }
    pub fn units_for_symbol(&self, symbol: &str) -> Result<Vec<Unit<'_>>, QuantityError<'_>> {
        let indexes = self
            .symbols
            .get(symbol)
            .ok_or_else(|| unknown(Record::UnitSymbol, symbol))?;
        Ok(indexes
            .iter()
            .map(|&index| Unit {
                registry: self,
                index,
            })
            .collect())
    }
    pub fn kinds_for_symbol(&self, symbol: &str) -> Result<Vec<Kind<'_>>, QuantityError<'_>> {
        let mut result = Vec::new();
        for unit in self.units_for_symbol(symbol)? {
            for kind in unit.kinds() {
                if !result.contains(&kind) {
                    result.push(kind);
                }
            }
        }
        Ok(result)
    }
    /// A document or caller boundary: a value written with a unit symbol. The
    /// kind may be left to the symbol when the symbol names exactly one.
    pub fn quantity_for_symbol<'r>(
        &'r self,
        value: f64,
        symbol: &str,
        kind: Option<Kind<'r>>,
    ) -> Result<Quantity<'r>, QuantityError<'r>> {
        let kind = match kind {
            Some(kind) => kind,
            None => {
                let kinds = self.kinds_for_symbol(symbol)?;
                match kinds.as_slice() {
                    [kind] => *kind,
                    [] | [_, _, ..] => {
                        return Err(QuantityError::AmbiguousKind {
                            symbol: symbol.into(),
                            kinds,
                        })
                    }
                }
            }
        };
        Quantity::new(value, self.symbol_unit(symbol, kind)?, kind)
    }
    /// A caller boundary: the one unit of `kind` written with `symbol`.
    pub fn symbol_unit<'r>(
        &'r self,
        symbol: &str,
        kind: Kind<'r>,
    ) -> Result<Unit<'r>, QuantityError<'r>> {
        if !std::ptr::eq(kind.registry, self) {
            return Err(QuantityError::RegistryMismatch);
        }
        let units: Vec<_> = self
            .units_for_symbol(symbol)?
            .into_iter()
            .filter(|unit| unit.kinds().any(|k| k == kind))
            .collect();
        match units.as_slice() {
            [unit] => Ok(*unit),
            [] => Err(QuantityError::SymbolKindMismatch {
                symbol: symbol.into(),
                kind,
            }),
            [_, _, ..] => Err(QuantityError::AmbiguousUnit {
                symbol: symbol.into(),
                kind,
                units,
            }),
        }
    }
}

fn unknown<'r>(record: Record, id: &str) -> QuantityError<'r> {
    QuantityError::Derivation(DerivationError::Unknown {
        record,
        id: id.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{derive, CatalogError, DerivationError, OperationParseError, Quantity, RateFault};

    const LENGTHS: &str = r#"
schema: 4
kinds:
  - {id: length, dimensions: {L: 1}}
  - {id: area, dimensions: {L: 2}}
units:
  - {id: cm, symbol: cm, kinds: [length], conversion: {reference_unit: cm, scale: '1'}, coherent_scale: '1/100'}
  - {id: m2, symbol: m2, kinds: [area], conversion: {reference_unit: m2, scale: '1'}, coherent_scale: '1'}
"#;
    fn refused(yaml: &str) -> CatalogError {
        Registry::from_yaml(yaml).unwrap_err()
    }
    /// LENGTHS with one declared row.
    fn with_row(row: &str) -> String {
        format!("{LENGTHS}operations:\n  - {row}\n")
    }
    #[test]
    fn products_use_coherent_scales_instead_of_reference_magnitudes() {
        let r = Registry::from_yaml(LENGTHS).unwrap();
        let length = r.unit("cm").unwrap().quantity(100.0).unwrap();
        let area = length.apply(Op::Mul, length).unwrap();
        assert_eq!(area.in_unit(r.unit("m2").unwrap()).unwrap(), 1.0);
        let (l, a) = (r.kind("length").unwrap(), r.kind("area").unwrap());
        assert_eq!(l.product(ProductOp::Mul, l), Ok(a));
        assert_eq!(a.product(ProductOp::Div, l), Ok(l));
    }
    #[test]
    fn a_restating_row_is_refused_whether_it_agrees_or_not() {
        assert_eq!(
            refused(&with_row(
                "{left: length, op: mul, right: length, result: area}"
            )),
            CatalogError::DerivedOperationRule {
                left: "length".into(),
                op: ProductOp::Mul,
                right: "length".into(),
                result: "area".into(),
                derived: "area".into(),
            }
        );
        // The dimensionless kind's neutral rule chooses energy over its twin.
        let neutral = r#"
schema: 4
dimensionless: unitless
kinds:
  - {id: unitless, dimensions: {}}
  - {id: energy, dimensions: {M: 1, L: 2, T: -2}}
  - {id: heat, dimensions: {M: 1, L: 2, T: -2}}
units: []
operations:
  - {left: unitless, op: mul, right: energy, result: heat}
"#;
        assert_eq!(
            refused(neutral),
            CatalogError::DerivedOperationRule {
                left: "unitless".into(),
                op: ProductOp::Mul,
                right: "energy".into(),
                result: "heat".into(),
                derived: "energy".into(),
            }
        );
    }
    #[test]
    fn a_dimensionally_wrong_row_is_refused() {
        let area = Dimensions::from_integer_powers([("L", 2)]);
        assert_eq!(
            refused(&with_row(
                "{left: length, op: mul, right: length, result: length}"
            )),
            CatalogError::InvalidOperationRule {
                left: "length".into(),
                op: ProductOp::Mul,
                right: "length".into(),
                result: "length".into(),
                dimensions: area.clone(),
                grade: Grade::Scalar,
            }
        );
        let bivector_area = with_row("{left: length, op: mul, right: length, result: area}")
            .replace("{L: 2}}", "{L: 2}, grade: 2}");
        assert_eq!(
            refused(&bivector_area),
            CatalogError::InvalidOperationRule {
                left: "length".into(),
                op: ProductOp::Mul,
                right: "length".into(),
                result: "area".into(),
                dimensions: area,
                grade: Grade::Scalar,
            }
        );
    }
    #[test]
    fn a_twin_row_is_kept_and_an_unresolved_twin_is_named() {
        let r = Registry::from_yaml(
            r#"
schema: 4
kinds:
  - {id: energy, dimensions: {M: 1, L: 2, T: -2}}
  - {id: torque, dimensions: {M: 1, L: 2, T: -2}}
  - {id: force, dimensions: {M: 1, L: 1, T: -2}}
  - {id: length, dimensions: {L: 1}}
units: []
operations:
  - {left: force, op: mul, right: length, result: energy}
"#,
        )
        .unwrap();
        let kind = |id| r.kind(id).unwrap();
        assert_eq!(
            kind("force").product(ProductOp::Mul, kind("length")),
            Ok(kind("energy"))
        );
        assert_eq!(
            kind("length").product(ProductOp::Mul, kind("force")),
            Err(QuantityError::UnresolvedTwin {
                left: kind("length"),
                op: ProductOp::Mul,
                right: kind("force"),
                twins: vec![kind("energy"), kind("torque")],
            })
        );
    }
    #[test]
    fn a_twin_row_keeps_its_provenance() {
        let yaml = r#"
schema: 4
kinds:
  - {id: energy, dimensions: {M: 1, L: 2, T: -2}}
  - {id: torque, dimensions: {M: 1, L: 2, T: -2}}
  - {id: force, dimensions: {M: 1, L: 1, T: -2}}
  - {id: length, dimensions: {L: 1}}
units: []
operations:
  - {left: force, op: mul, right: length, result: energy, provenance: 'Work: W = Fd'}
"#;
        let (r, other) = (
            Registry::from_yaml(yaml).unwrap(),
            Registry::from_yaml(yaml).unwrap(),
        );
        let kind = |id| r.kind(id).unwrap();
        assert_eq!(
            kind("force").row_provenance(ProductOp::Mul, kind("length")),
            Ok(Some("Work: W = Fd"))
        );
        assert_eq!(
            kind("length").row_provenance(ProductOp::Mul, kind("force")),
            Ok(None)
        );
        assert_eq!(
            kind("force").row_provenance(ProductOp::Mul, other.kind("length").unwrap()),
            Err(QuantityError::RegistryMismatch)
        );
    }
    #[test]
    fn a_power_names_its_twins() {
        let r = Registry::from_yaml(&LENGTHS.replace(
            "  - {id: area, dimensions: {L: 2}}",
            "  - {id: area, dimensions: {L: 2}}\n  - {id: cross_section, dimensions: {L: 2}}",
        ))
        .unwrap();
        let kind = |id| r.kind(id).unwrap();
        let two = BigRational::from_integer(2.into());
        assert_eq!(
            kind("length").power(&two),
            Err(QuantityError::UnresolvedPowerTwin {
                base: kind("length"),
                exponent: two.clone(),
                twins: vec![kind("area"), kind("cross_section")],
            })
        );
    }
    const RATES: &str = r#"
schema: 4
time: time
kinds:
  - {id: time, dimensions: {T: 1}, difference_kind: duration}
  - {id: duration, dimensions: {T: 1}}
  - {id: momentum, dimensions: {M: 1, L: 1, T: -1}}
  - {id: impulse, dimensions: {M: 1, L: 1, T: -1}}
  - {id: force, dimensions: {M: 1, L: 1, T: -2}, rate_of: momentum}
units: []
"#;
    #[test]
    fn a_rate_resolves_its_twin_over_a_duration() {
        let r = Registry::from_yaml(RATES).unwrap();
        let kind = |id| r.kind(id).unwrap();
        let (force, duration) = (kind("force"), kind("duration"));
        assert_eq!(
            force.product(ProductOp::Mul, duration),
            Ok(kind("momentum"))
        );
        assert_eq!(
            duration.product(ProductOp::Mul, force),
            Ok(kind("momentum"))
        );
        assert_eq!(
            kind("momentum").product(ProductOp::Div, duration),
            Ok(force)
        );
        assert_eq!(kind("impulse").product(ProductOp::Div, duration), Ok(force));
        let restated = format!(
            "{RATES}operations:\n  - {{left: force, op: mul, right: duration, result: impulse}}\n"
        );
        assert_eq!(
            refused(&restated),
            CatalogError::DerivedOperationRule {
                left: "force".into(),
                op: ProductOp::Mul,
                right: "duration".into(),
                result: "impulse".into(),
                derived: "momentum".into(),
            }
        );
    }
    #[test]
    fn rate_declarations_are_checked() {
        let fault = |yaml: &str| match refused(yaml) {
            CatalogError::InvalidRate { fault, .. } => fault,
            other => panic!("expected InvalidRate, got {other:?}"),
        };
        assert_eq!(
            refused(&RATES.replace("time: time\n", "")),
            CatalogError::InvalidRate {
                rate: "force".into(),
                of: "momentum".into(),
                fault: RateFault::NoTimeKind
            }
        );
        assert_eq!(
            fault(&RATES.replace("rate_of: momentum", "rate_of: time")),
            RateFault::PointKind("time".into())
        );
        assert_eq!(
            fault(&RATES.replace("rate_of: momentum", "rate_of: duration")),
            RateFault::Mismatch {
                dimensions: Dimensions::from_integer_powers([("M", 1), ("L", 1), ("T", -1)]),
                grade: Grade::Scalar
            }
        );
        let second = RATES.replace(
            "units: []",
            "  - {id: thrust, dimensions: {M: 1, L: 1, T: -2}, rate_of: momentum}\nunits: []",
        );
        assert_eq!(fault(&second), RateFault::AlsoRateOf("force".into()));
        assert_eq!(
            refused(&RATES.replace("time: time", "time: duration")),
            CatalogError::InvalidTimeKind("duration".into())
        );
    }
    #[test]
    fn invalid_declarations_are_refused_with_their_cause() {
        let commutative_division =
            with_row("{left: length, op: div, right: length, result: area, commutative: true}");
        assert_eq!(
            refused(&commutative_division),
            CatalogError::CommutativeQuotient {
                left: "length".into(),
                right: "length".into()
            }
        );
        let point_result = with_row("{left: length, op: mul, right: length, result: spot}")
            .replace(
                "  - {id: area, dimensions: {L: 2}}",
                "  - {id: area, dimensions: {L: 2}}\n  - {id: spot, dimensions: {L: 2}, difference_kind: area}",
            );
        assert_eq!(
            refused(&point_result),
            CatalogError::Derivation(DerivationError::Point {
                left: "length".into(),
                op: ProductOp::Mul,
                right: "length".into(),
                point: "spot".into()
            })
        );
        let scalar_dot = with_row("{left: length, op: dot, right: area, result: area}");
        assert_eq!(
            refused(&scalar_dot),
            CatalogError::Derivation(DerivationError::Ungraded {
                left: "length".into(),
                op: ProductOp::Dot,
                right: "area".into(),
                left_grade: Grade::Scalar,
                right_grade: Grade::Scalar
            })
        );
        let zero = LENGTHS.replace("scale: '1'}, coherent_scale: '1/100'", "scale: '0'}");
        assert_eq!(
            refused(&zero),
            CatalogError::ZeroScale { unit: "cm".into() }
        );
        let additive = with_row("{left: length, op: add, right: length, result: area}");
        assert!(matches!(refused(&additive), CatalogError::Yaml(_)));
        assert_eq!(
            "add".parse::<ProductOp>(),
            Err(OperationParseError::NotProduct(Op::Add))
        );
        let crossed = LENGTHS.replace("reference_unit: cm", "reference_unit: m2");
        assert!(matches!(
            refused(&crossed),
            CatalogError::IncompatibleReference { .. }
        ));
        let nested = LENGTHS.replace(
            "{id: length, dimensions: {L: 1}}",
            "{id: length, dimensions: {L: 1}, difference_kind: length}",
        );
        assert_eq!(
            refused(&nested),
            CatalogError::NestedAffineSpace {
                point: "length".into(),
                difference: "length".into()
            }
        );
        let dimensional = format!("{LENGTHS}dimensionless: length\n");
        assert_eq!(
            refused(&dimensional),
            CatalogError::InvalidDimensionlessKind("length".into())
        );
        // A least value is stated in the canonical unit (m2), which m2b does not reach.
        let second = "  - {id: m2b, symbol: m2b, kinds: [area], conversion: {reference_unit: m2b, scale: '1'}, coherent_scale: '1'}\n";
        let unreachable = format!("{LENGTHS}{second}").replace("{L: 2}}", "{L: 2}, minimum: '0'}");
        assert_eq!(
            refused(&unreachable),
            CatalogError::InvalidMinimum {
                kind: "area".into()
            }
        );
    }
    #[test]
    fn a_difference_is_the_declared_difference_kind_of_a_point_and_else_the_kind() {
        let r = Registry::from_yaml(RATES).unwrap();
        let kind = |id| r.kind(id).unwrap();
        assert_eq!(kind("time").difference(), kind("duration"));
        assert_eq!(kind("duration").difference(), kind("duration"));
        assert_eq!(kind("force").difference(), kind("force"));
        for k in r.kinds() {
            assert_eq!(k.combine(Op::Sub, k), Ok(k.difference()), "{k}");
        }
    }
    #[test]
    fn a_kind_is_derived_through_its_operand() {
        let r = Registry::from_yaml(RATES).unwrap();
        let kind = |id| r.kind(id).unwrap();
        let (force, duration) = (
            kind("force").operand().unwrap(),
            kind("duration").operand().unwrap(),
        );
        assert_eq!(
            derive(&force, ProductOp::Mul, &duration).map(|graded| graded.dimensions),
            Ok(kind("momentum").dimensions().unwrap().clone())
        );
        assert_eq!(kind("time").operand().unwrap().role, AffineRole::Point);
    }
    #[test]
    fn compile_and_kinds_report_one_derivation_error() {
        let undimensioned = "schema: 4\nkinds:\n  - {id: length, dimensions: {L: 1}}\n  - {id: blob, dimensions: null}\nunits: []\n";
        let r = Registry::from_yaml(undimensioned).unwrap();
        let (length, blob) = (r.kind("length").unwrap(), r.kind("blob").unwrap());
        let Err(QuantityError::Derivation(for_quantities)) = length.product(ProductOp::Mul, blob)
        else {
            panic!("expected a derivation error");
        };
        assert_eq!(
            for_quantities,
            DerivationError::UnresolvedDimensions { kind: blob }
        );
        let row = format!("{undimensioned}operations:\n  - {{left: length, op: mul, right: blob, result: length}}\n");
        assert_eq!(
            refused(&row.replace(
                "{id: length, dimensions: {L: 1}}",
                "{id: length, dimensions: null}"
            )),
            CatalogError::Derivation(DerivationError::UnresolvedDimensions {
                kind: "length".into()
            })
        );
        assert_eq!(
            refused(&row.replace("result: length", "result: other")),
            CatalogError::Derivation(DerivationError::Unknown {
                record: crate::Record::Kind,
                id: "other".into()
            })
        );
        assert_eq!(
            for_quantities.map_kinds(|kind| kind.id().to_owned()),
            DerivationError::UnresolvedDimensions {
                kind: "blob".into()
            }
        );
    }
    #[test]
    fn operation_names_are_spelled_once() {
        for op in Op::ALL {
            assert_eq!(op.name().parse::<Op>(), Ok(op));
            assert_eq!(
                serde_json::to_string(&op).unwrap(),
                format!("\"{}\"", op.name())
            );
        }
    }
    #[test]
    fn same_kind_symbol_collision_requires_a_unit_handle() {
        let r = Registry::from_json(
            r#"{
          "schema":4,"kinds":[{"id":"length","dimensions":{"L":"1"}}],
          "units":[
            {"id":"u1","symbol":"u","kinds":["length"],"conversion":{"reference_unit":"u1","scale":"1"}},
            {"id":"u2","symbol":"u","kinds":["length"],"conversion":{"reference_unit":"u1","scale":"2"}}
          ]
        }"#,
        )
        .unwrap();
        let length = r.kind("length").unwrap();
        assert_eq!(
            r.quantity_for_symbol(1.0, "u", Some(length)).err(),
            Some(QuantityError::AmbiguousUnit {
                symbol: "u".into(),
                kind: length,
                units: vec![r.unit("u1").unwrap(), r.unit("u2").unwrap()],
            })
        );
        let two = Quantity::new(1.0, r.unit("u2").unwrap(), length).unwrap();
        assert_eq!(two.in_unit(r.unit("u1").unwrap()).unwrap(), 2.0);
    }
    #[test]
    fn a_linear_kind_cannot_silently_discard_an_offset() {
        let r = Registry::from_json(r#"{
          "schema":4,"kinds":[{"id":"coordinate","dimensions":{"X":1}}],
          "units":[
            {"id":"base","symbol":"base","kinds":["coordinate"],"conversion":{"reference_unit":"base","scale":"1"}},
            {"id":"shifted","symbol":"shifted","kinds":["coordinate"],"conversion":{"reference_unit":"base","scale":"1","offset":["10"]}}
          ]
        }"#).unwrap();
        let (base, shifted) = (r.unit("base").unwrap(), r.unit("shifted").unwrap());
        let coordinate = r.kind("coordinate").unwrap();
        let refused = QuantityError::OffsetOnLinearKind {
            unit: shifted,
            kind: coordinate,
        };
        assert_eq!(shifted.quantity(2.0).unwrap_err(), refused);
        // Reading a value out and converting it exactly refuse the same offset.
        assert_eq!(
            base.quantity(2.0).unwrap().in_unit(shifted).unwrap_err(),
            refused
        );
        assert_eq!(
            coordinate
                .convert_exact(ExactValue::default(), base, shifted)
                .unwrap_err(),
            refused
        );
    }
    #[test]
    fn a_symbol_names_one_unit_of_a_kind() {
        let r = Registry::from_yaml(RATES.replace(
            "units: []",
            "units:\n  - {id: second, symbol: s, kinds: [time], conversion: {reference_unit: second, scale: '1'}}\n  - {id: second_delta, symbol: s, kinds: [duration], conversion: {reference_unit: second_delta, scale: '1'}}",
        ).as_str())
        .unwrap();
        let kind = |id| r.kind(id).unwrap();
        assert_eq!(
            r.symbol_unit("s", kind("duration")),
            Ok(r.unit("second_delta").unwrap())
        );
        assert_eq!(
            r.symbol_unit("s", kind("force")),
            Err(QuantityError::SymbolKindMismatch {
                symbol: "s".into(),
                kind: kind("force")
            })
        );
        let elapsed = r
            .quantity_for_symbol(2.0, "s", Some(kind("duration")))
            .unwrap();
        assert_eq!(elapsed.in_symbol("s"), Ok(2.0));
    }
    #[test]
    fn approximate_magnitudes_convert_but_refuse_exact_conversion() {
        let r = Registry::from_json(r#"{"schema":4,"kinds":[{"id":"x","dimensions":{}}],"units":[
            {"id":"u","symbol":"u","kinds":["x"],"conversion":{"reference_unit":"u","scale":"1"}},
            {"id":"v","symbol":"v","kinds":["x"],"conversion":{"reference_unit":"u","scale":{"approximate":2.5}}}
          ]}"#).unwrap();
        let (u, v) = (r.unit("u").unwrap(), r.unit("v").unwrap());
        assert_eq!(v.quantity(2.0).unwrap().in_unit(u).unwrap(), 5.0);
        assert_eq!(
            r.kind("x")
                .unwrap()
                .convert_exact(ExactValue::default(), v, u),
            Err(QuantityError::ApproximateConversion { unit: v })
        );
    }
    #[test]
    fn handles_of_another_registry_are_refused() {
        let (a, b) = (
            Registry::from_yaml(LENGTHS).unwrap(),
            Registry::from_yaml(LENGTHS).unwrap(),
        );
        let (x, y) = (a.kind("length").unwrap(), b.kind("length").unwrap());
        assert_ne!(x, y);
        assert_eq!(x.combine(Op::Add, y), Err(QuantityError::RegistryMismatch));
        assert_eq!(
            Quantity::new(1.0, a.unit("cm").unwrap(), y).err(),
            Some(QuantityError::RegistryMismatch)
        );
        assert_eq!(format!("{x:?} {x}"), "Kind(\"length\") length");
    }
}
