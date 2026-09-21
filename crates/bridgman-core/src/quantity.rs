use crate::{AffineRole, ExactScalar, KindHandle, Op, QuantityError, Registry, UnitHandle};
use num_traits::Zero;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExactValue {
    terms: BTreeMap<num_bigint::BigInt, num_rational::BigRational>,
}
impl ExactValue {
    pub fn from_scalar(value: ExactScalar) -> Self {
        Self::from_terms(vec![value])
    }
    pub(crate) fn from_terms(terms: Vec<ExactScalar>) -> Self {
        let mut result = Self::default();
        for term in terms {
            result.add_term(term)
        }
        result
    }
    pub fn terms(&self) -> impl Iterator<Item = ExactScalar> + '_ {
        self.terms
            .iter()
            .map(|(pi_exponent, rational)| ExactScalar {
                rational: rational.clone(),
                pi_exponent: pi_exponent.clone(),
            })
    }
    fn add_term(&mut self, value: ExactScalar) {
        let total = self
            .terms
            .get(&value.pi_exponent)
            .cloned()
            .unwrap_or_else(num_rational::BigRational::zero)
            + value.rational;
        if total.is_zero() {
            self.terms.remove(&value.pi_exponent);
        } else {
            self.terms.insert(value.pi_exponent, total);
        }
    }
    pub fn add(&self, rhs: &Self) -> Self {
        let mut result = self.clone();
        for term in rhs.terms() {
            result.add_term(term)
        }
        result
    }
    pub fn sub(&self, rhs: &Self) -> Self {
        let mut result = self.clone();
        for mut term in rhs.terms() {
            term.rational = -term.rational;
            result.add_term(term)
        }
        result
    }
    pub fn multiply_scalar(&self, rhs: &ExactScalar) -> Self {
        let mut result = Self::default();
        for term in self.terms() {
            result.add_term(term.multiply(rhs));
        }
        result
    }
    pub fn divide_scalar(&self, rhs: &ExactScalar) -> Result<Self, QuantityError> {
        if rhs.rational.is_zero() {
            return Err(QuantityError::DivisionByZero);
        }
        let mut result = Self::default();
        for term in self.terms() {
            result.add_term(term.divide(rhs).ok_or(QuantityError::DivisionByZero)?);
        }
        Ok(result)
    }
    pub fn to_f64(&self) -> Result<f64, QuantityError> {
        let mut result = 0.0;
        for term in self.terms() {
            result += term.to_f64().ok_or(QuantityError::NumericalFailure)?;
        }
        if result.is_finite() {
            Ok(result)
        } else {
            Err(QuantityError::NumericalFailure)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicQuantity {
    pub(crate) registry: u64,
    pub(crate) kind: KindHandle,
    pub(crate) role: AffineRole,
    pub(crate) reference_unit: UnitHandle,
    pub(crate) value: f64,
}
impl DynamicQuantity {
    pub fn kind(self) -> KindHandle {
        self.kind
    }
    pub fn role(self) -> AffineRole {
        self.role
    }
    pub fn in_unit(self, registry: &Registry, unit: UnitHandle) -> Result<f64, QuantityError> {
        registry.check_quantity(self, unit)?;
        let (reference, scale, offset) = registry.conversion(unit)?;
        if reference != self.reference_unit {
            return Err(QuantityError::DisconnectedConversion);
        }
        let value = (self.value
            - if self.role == AffineRole::Point {
                offset
            } else {
                0.0
            })
            / scale;
        if value.is_finite() {
            Ok(value)
        } else {
            Err(QuantityError::NumericalFailure)
        }
    }
    pub fn add(self, registry: &Registry, rhs: Self) -> Result<Self, QuantityError> {
        registry.binary(self, Op::Add, rhs)
    }
    pub fn sub(self, registry: &Registry, rhs: Self) -> Result<Self, QuantityError> {
        registry.binary(self, Op::Sub, rhs)
    }
    pub fn mul(self, registry: &Registry, rhs: Self) -> Result<Self, QuantityError> {
        registry.binary(self, Op::Mul, rhs)
    }
    pub fn div(self, registry: &Registry, rhs: Self) -> Result<Self, QuantityError> {
        registry.binary(self, Op::Div, rhs)
    }
}

impl Registry {
    pub fn quantity(
        &self,
        value: f64,
        unit: UnitHandle,
        kind: KindHandle,
        role: AffineRole,
    ) -> Result<DynamicQuantity, QuantityError> {
        if !value.is_finite() {
            return Err(QuantityError::NonFiniteInput);
        }
        self.check_handles(kind, unit)?;
        self.require_unit_kind(unit, kind)?;
        self.dimensions(kind)?;
        let (reference_unit, scale, offset) = self.conversion(unit)?;
        let value = scale * value
            + if role == AffineRole::Point {
                offset
            } else {
                0.0
            };
        if !value.is_finite() {
            return Err(QuantityError::NumericalFailure);
        }
        Ok(DynamicQuantity {
            registry: self.identity(),
            kind,
            role,
            reference_unit,
            value,
        })
    }
    pub fn quantity_for_symbol(
        &self,
        value: f64,
        symbol: &str,
        kind: Option<KindHandle>,
        role: AffineRole,
    ) -> Result<DynamicQuantity, QuantityError> {
        let units = self.units_for_symbol(symbol)?;
        let resolved_kind = if let Some(k) = kind {
            self.check_kind_public(k)?;
            k
        } else {
            let kinds = self.kinds_for_symbol(symbol)?;
            if kinds.len() != 1 {
                return Err(QuantityError::AmbiguousKind(symbol.into()));
            }
            kinds[0]
        };
        let mut candidates = units
            .into_iter()
            .filter(|u| self.unit_has_kind(*u, resolved_kind));
        let selected = candidates
            .next()
            .ok_or_else(|| QuantityError::UnitKindMismatch {
                unit: symbol.into(),
                kind: self.kind_id(resolved_kind).unwrap_or("?").into(),
            })?;
        if candidates.next().is_some() {
            return Err(QuantityError::AmbiguousUnit(symbol.into()));
        }
        self.quantity(value, selected, resolved_kind, role)
    }
    pub fn convert_exact(
        &self,
        value: ExactValue,
        from: UnitHandle,
        to: UnitHandle,
        role: AffineRole,
    ) -> Result<ExactValue, QuantityError> {
        self.check_unit_public(from)?;
        self.check_unit_public(to)?;
        let (from_ref, from_scale, from_offset) = self.exact_conversion(from)?;
        let (to_ref, to_scale, to_offset) = self.exact_conversion(to)?;
        if from_ref != to_ref {
            return Err(QuantityError::DisconnectedConversion);
        }
        let mut reference = value.multiply_scalar(&from_scale);
        if role == AffineRole::Point {
            reference = reference.add(&from_offset)
        }
        if role == AffineRole::Point {
            reference = reference.sub(&to_offset)
        }
        reference.divide_scalar(&to_scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Catalog, Dimensions, KindDecl, OperationDecl, UnitDecl, CATALOG_SCHEMA};
    use std::collections::BTreeMap;
    fn scalar(v: &str) -> ExactScalar {
        ExactScalar::parse(v).unwrap()
    }
    fn unit(
        id: &str,
        symbol: &str,
        kinds: &[&str],
        reference: &str,
        scale: &str,
        offset: &str,
    ) -> UnitDecl {
        UnitDecl {
            id: id.into(),
            symbol: symbol.into(),
            kinds: kinds.iter().map(|x| (*x).into()).collect(),
            reference_unit: Some(reference.into()),
            scale: Some(scalar(scale)),
            coherent_scale: Some(scalar(scale)),
            approximate_scale: None,
            offset: scalar(offset),
            offset_terms: vec![],
            approximate_offset: None,
        }
    }
    fn registry() -> Registry {
        let dims = Dimensions::from_integer_powers([("Theta", 1)]);
        let energy = Dimensions::from_integer_powers([("M", 1), ("L", 2), ("T", -2)]);
        Registry::compile(Catalog {
            schema: CATALOG_SCHEMA,
            provenance: BTreeMap::new(),
            kinds: vec![
                KindDecl {
                    id: "temperature".into(),
                    dimensions: Some(dims.clone()),
                    difference_kind: Some("temperature_difference".into()),
                },
                KindDecl {
                    id: "temperature_difference".into(),
                    dimensions: Some(dims),
                    difference_kind: None,
                },
                KindDecl {
                    id: "mass".into(),
                    dimensions: Some(Dimensions::from_integer_powers([("M", 1)])),
                    difference_kind: None,
                },
                KindDecl {
                    id: "energy".into(),
                    dimensions: Some(energy.clone()),
                    difference_kind: None,
                },
                KindDecl {
                    id: "torque".into(),
                    dimensions: Some(energy),
                    difference_kind: None,
                },
                KindDecl {
                    id: "mass_squared".into(),
                    dimensions: Some(Dimensions::from_integer_powers([("M", 2)])),
                    difference_kind: None,
                },
                KindDecl {
                    id: "angle".into(),
                    dimensions: Some(Dimensions::one()),
                    difference_kind: None,
                },
                KindDecl {
                    id: "widget".into(),
                    dimensions: Some(Dimensions::one()),
                    difference_kind: None,
                },
            ],
            units: vec![
                unit(
                    "kelvin",
                    "K",
                    &["temperature", "temperature_difference"],
                    "kelvin",
                    "1",
                    "0",
                ),
                unit(
                    "celsius",
                    "degC",
                    &["temperature", "temperature_difference"],
                    "kelvin",
                    "1",
                    "27315/100",
                ),
                unit(
                    "millicelsius",
                    "mdegC",
                    &["temperature", "temperature_difference"],
                    "kelvin",
                    "1/1000",
                    "27315/100",
                ),
                unit("gram", "g", &["mass"], "gram", "1", "0"),
                unit("kilogram", "kg", &["mass"], "gram", "1000", "0"),
                unit(
                    "mass_squared",
                    "g2",
                    &["mass_squared"],
                    "mass_squared",
                    "1",
                    "0",
                ),
                unit("joule", "J", &["energy"], "joule", "1", "0"),
                unit("joule_product", "N*m", &["energy"], "joule", "1", "0"),
                unit("newton_metre", "N*m", &["torque"], "newton_metre", "1", "0"),
                unit("radian", "rad", &["angle"], "radian", "1", "0"),
                unit("degree", "deg", &["angle"], "radian", "1/180*pi^1", "0"),
                unit("widget", "widget", &["widget"], "widget", "1", "0"),
            ],
            operations: vec![OperationDecl {
                left: "mass".into(),
                op: Op::Mul,
                right: "mass".into(),
                result: "mass_squared".into(),
                commutative: false,
                provenance: None,
            }],
        })
        .unwrap()
    }
    #[test]
    fn point_and_difference_conversions_follow_declared_roles() {
        let r = registry();
        let t = r.kind("temperature").unwrap();
        let d = r.kind("temperature_difference").unwrap();
        let k = r.unit("kelvin").unwrap();
        let c = r.unit("celsius").unwrap();
        let mc = r.unit("millicelsius").unwrap();
        assert_eq!(
            r.quantity(20.0, c, t, AffineRole::Point)
                .unwrap()
                .in_unit(&r, k)
                .unwrap(),
            293.15
        );
        assert_eq!(
            r.quantity(20.0, c, d, AffineRole::Difference)
                .unwrap()
                .in_unit(&r, k)
                .unwrap(),
            20.0
        );
        assert_eq!(
            r.quantity(1000.0, mc, t, AffineRole::Point)
                .unwrap()
                .in_unit(&r, k)
                .unwrap(),
            274.15
        );
        let a = r.quantity(30.0, c, t, AffineRole::Point).unwrap();
        let b = r.quantity(20.0, c, t, AffineRole::Point).unwrap();
        let delta = a.sub(&r, b).unwrap();
        assert_eq!(delta.role(), AffineRole::Difference);
        assert_eq!(delta.in_unit(&r, k).unwrap(), 10.0);
        assert_eq!(a.add(&r, b), Err(QuantityError::UnsupportedAffineOperation));
    }
    #[test]
    fn reference_unit_is_not_applied_twice() {
        let r = registry();
        let q = r
            .quantity(
                1.0,
                r.unit("kilogram").unwrap(),
                r.kind("mass").unwrap(),
                AffineRole::Linear,
            )
            .unwrap();
        assert_eq!(q.in_unit(&r, r.unit("gram").unwrap()).unwrap(), 1000.0);
    }
    #[test]
    fn exact_pi_conversion_remains_symbolic() {
        let r = registry();
        let value = ExactValue::from_scalar(scalar("180"));
        let converted = r
            .convert_exact(
                value,
                r.unit("degree").unwrap(),
                r.unit("radian").unwrap(),
                AffineRole::Linear,
            )
            .unwrap();
        assert_eq!(
            converted.terms().map(|v| v.encoded()).collect::<Vec<_>>(),
            vec!["1*pi^1"]
        );
    }
    #[test]
    fn semantic_twins_and_ambiguous_symbols_do_not_collapse() {
        let r = registry();
        let e = r
            .quantity(
                1.0,
                r.unit("joule").unwrap(),
                r.kind("energy").unwrap(),
                AffineRole::Linear,
            )
            .unwrap();
        let t = r
            .quantity(
                1.0,
                r.unit("newton_metre").unwrap(),
                r.kind("torque").unwrap(),
                AffineRole::Linear,
            )
            .unwrap();
        assert!(matches!(
            e.add(&r, t),
            Err(QuantityError::KindMismatch { .. })
        ));
        assert_eq!(
            r.quantity_for_symbol(1.0, "N*m", None, AffineRole::Linear),
            Err(QuantityError::AmbiguousKind("N*m".into()))
        );
    }
    #[test]
    fn affine_overflow_is_an_error() {
        let r = registry();
        let point = |value| {
            r.quantity(
                value,
                r.unit("kelvin").unwrap(),
                r.kind("temperature").unwrap(),
                AffineRole::Point,
            )
            .unwrap()
        };
        let delta = r
            .quantity(
                1e308,
                r.unit("kelvin").unwrap(),
                r.kind("temperature_difference").unwrap(),
                AffineRole::Difference,
            )
            .unwrap();
        assert_eq!(
            point(1e308).sub(&r, point(-1e308)),
            Err(QuantityError::NumericalFailure)
        );
        assert_eq!(
            point(1e308).add(&r, delta),
            Err(QuantityError::NumericalFailure)
        );
    }
    #[test]
    fn difference_plus_point_is_commutative() {
        let r = registry();
        let point = r
            .quantity(
                20.0,
                r.unit("celsius").unwrap(),
                r.kind("temperature").unwrap(),
                AffineRole::Point,
            )
            .unwrap();
        let delta = r
            .quantity(
                10.0,
                r.unit("kelvin").unwrap(),
                r.kind("temperature_difference").unwrap(),
                AffineRole::Difference,
            )
            .unwrap();
        assert_eq!(delta.add(&r, point), point.add(&r, delta));
    }
    #[test]
    fn zero_exact_value_cannot_be_divided_by_zero() {
        assert_eq!(
            ExactValue::default().divide_scalar(&ExactScalar::zero()),
            Err(QuantityError::DivisionByZero)
        );
    }
    #[test]
    fn finite_overflow_is_an_error() {
        let r = registry();
        let mass = r.kind("mass").unwrap();
        let a = r
            .quantity(1e308, r.unit("gram").unwrap(), mass, AffineRole::Linear)
            .unwrap();
        assert_eq!(a.mul(&r, a), Err(QuantityError::NumericalFailure));
    }
}
