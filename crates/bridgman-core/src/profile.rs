//! The bundled thermal catalog, declared as data in `profiles/thermal.yml` and
//! compiled once. Documents read kinds and quantities against it: a `'static`
//! kind is written by its id, and a `'static` quantity as a value and unit
//! symbol.
//!
//! ```
//! use bridgman_core::{Op, profile::registry};
//! let r = registry();
//! let capacity = r.quantity_for_symbol(2.0, "kg", None)?
//!     .apply(Op::Mul, r.quantity_for_symbol(500.0, "J/(kg*K)", None)?)?;
//! let heat = capacity.apply(Op::Mul, r.quantity_for_symbol(100.0, "delta_K", None)?)?;
//! assert_eq!(heat.in_symbol("J")?, 100000.0);
//! // Energy and torque share dimensions but are different kinds.
//! let torque = r.quantity_for_symbol(1.0, "N*m", None)?;
//! assert!(heat.apply(Op::Add, torque).is_err());
//! # Ok::<(), bridgman_core::QuantityError>(())
//! ```
use crate::{Kind, Quantity, QuantityError, Registry};
use serde::{de::Error as _, Deserialize, Deserializer};
use std::sync::OnceLock;

pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Registry::from_yaml(include_str!("../../../profiles/thermal.yml"))
            .expect("the bundled thermal catalog compiles")
    })
}

/// A value and unit symbol as written, not yet checked. A declaration converts
/// it while parsing; a question may keep it, so that a nonfinite value or an
/// unknown unit becomes that question's outcome instead of a document error.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stated {
    pub value: f64,
    pub unit: String,
}
impl TryFrom<Stated> for Quantity<'static> {
    type Error = QuantityError;
    fn try_from(stated: Stated) -> Result<Self, QuantityError> {
        registry().quantity_for_symbol(stated.value, &stated.unit, None)
    }
}
impl<'de> Deserialize<'de> for Quantity<'static> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Stated::deserialize(d)?.try_into().map_err(D::Error::custom)
    }
}
impl<'de> Deserialize<'de> for Kind<'static> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let id = String::deserialize(d)?;
        registry().kind(&id).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AffineRole, Catalog, Dimensions, ExactScalar, Op, Operation, ProductOp};
    use std::cmp::Ordering;

    fn q(value: f64, symbol: &str) -> Quantity<'static> {
        registry().quantity_for_symbol(value, symbol, None).unwrap()
    }
    #[test]
    fn declared_products_and_quotients_keep_kinds() {
        let capacity = q(2.0, "kg").apply(Op::Mul, q(500.0, "J/(kg*K)")).unwrap();
        assert_eq!(capacity.kind().id(), "heat_capacity");
        let heat = capacity.apply(Op::Mul, q(100.0, "delta_K")).unwrap();
        assert_eq!(heat.in_symbol("kJ").unwrap(), 100.0);
        let back = heat.apply(Op::Div, capacity).unwrap();
        assert_eq!(back.in_symbol("delta_degF").unwrap(), 180.0);
        // Units convert through coherent scales, whatever the operands' units.
        let grams = q(2000.0, "g").apply(Op::Mul, q(0.5, "kJ/(kg*K)")).unwrap();
        assert_eq!(grams, capacity);
    }
    #[test]
    fn affine_temperatures_follow_their_declared_space() {
        let cold = q(0.0, "degC");
        let hot = q(212.0, "degF");
        let delta = hot.apply(Op::Sub, cold).unwrap();
        assert_eq!(delta.kind().id(), "temperature_delta");
        assert!((delta.in_symbol("delta_K").unwrap() - 100.0).abs() < 1e-10);
        let warmed = cold.apply(Op::Add, delta).unwrap();
        assert!((warmed.in_symbol("degC").unwrap() - 100.0).abs() < 1e-10);
        assert_eq!(delta.apply(Op::Add, cold).unwrap(), warmed);
        assert!(matches!(
            cold.apply(Op::Add, hot),
            Err(QuantityError::UnsupportedOperation { .. })
        ));
        assert_eq!(
            q(0.0, "K").apply(Op::Sub, q(1.0, "delta_K")),
            Err(QuantityError::BelowMinimum {
                kind: "temperature".into(),
                unit: "kelvin".into(),
                minimum: ExactScalar::zero(),
                value: ExactScalar::parse("-1").unwrap(),
            })
        );
        assert!(matches!(
            q(300.0, "K").scale(2.0),
            Err(QuantityError::UnsupportedOperation {
                operation: Operation::Scale,
                ..
            })
        ));
        assert_eq!(
            registry().kind("temperature").unwrap().role(),
            AffineRole::Point
        );
    }
    #[test]
    fn exact_conversion_keeps_each_kinds_affine_role() {
        use crate::ExactValue;
        let r = registry();
        let twenty = || ExactValue::from_scalar(ExactScalar::parse("20").unwrap());
        let point = r.kind("temperature").unwrap().convert_exact(
            twenty(),
            r.unit("degree_celsius").unwrap(),
            r.unit("kelvin").unwrap(),
        );
        let exact = ExactValue::from_scalar(ExactScalar::parse("5863/20").unwrap());
        assert_eq!(point, Ok(exact));
        let difference = r.kind("temperature_delta").unwrap().convert_exact(
            twenty(),
            r.unit("degree_celsius_delta").unwrap(),
            r.unit("kelvin_delta").unwrap(),
        );
        assert_eq!(difference, Ok(twenty()));
        assert!(matches!(
            r.kind("energy").unwrap().convert_exact(
                twenty(),
                r.unit("joule").unwrap(),
                r.unit("newton_metre").unwrap(),
            ),
            Err(QuantityError::UnitKindMismatch { .. })
        ));
    }
    #[test]
    fn affine_overflow_is_an_error() {
        let hot = q(1e308, "K");
        assert_eq!(
            hot.apply(Op::Add, q(1e308, "delta_K")),
            Err(QuantityError::NumericalFailure)
        );
        assert_eq!(
            q(1e308, "delta_K").apply(Op::Sub, q(-1e308, "delta_K")),
            Err(QuantityError::NumericalFailure)
        );
    }
    #[test]
    fn equal_dimensions_do_not_make_kinds_interchangeable() {
        let r = registry();
        let (energy, torque) = (r.kind("energy").unwrap(), r.kind("torque").unwrap());
        assert_eq!(energy.dimensions(), torque.dimensions());
        assert_eq!(
            q(1.0, "J").apply(Op::Add, q(1.0, "N*m")),
            Err(QuantityError::KindMismatch {
                expected: "energy".into(),
                actual: "torque".into()
            })
        );
        assert!(q(1.0, "J").compare(q(1.0, "N*m")).is_err());
        assert_eq!(
            r.kind("unitless").unwrap().dimensions().unwrap(),
            &Dimensions::one()
        );
    }
    #[test]
    fn the_dimensionless_kind_scales_and_cancels() {
        let heat = q(3.0, "J");
        let ratio = heat.apply(Op::Div, q(1.5, "J")).unwrap();
        assert_eq!(ratio.kind().id(), "unitless");
        assert_eq!(ratio.in_symbol("1").unwrap(), 2.0);
        assert_eq!(heat.apply(Op::Mul, ratio).unwrap(), q(6.0, "J"));
        assert!(matches!(
            q(1.0, "J").apply(Op::Mul, q(1.0, "J")),
            Err(QuantityError::NoProductKind { .. })
        ));
    }
    /// Every product the thermal profile once declared as a row.
    const THERMAL_PRODUCTS: [(&str, ProductOp, &str, &str); 17] = [
        ("mass", ProductOp::Mul, "specific_heat", "heat_capacity"),
        ("specific_heat", ProductOp::Mul, "mass", "heat_capacity"),
        (
            "heat_capacity",
            ProductOp::Mul,
            "temperature_delta",
            "energy",
        ),
        (
            "temperature_delta",
            ProductOp::Mul,
            "heat_capacity",
            "energy",
        ),
        ("mass", ProductOp::Mul, "specific_energy", "energy"),
        ("specific_energy", ProductOp::Mul, "mass", "energy"),
        ("length", ProductOp::Mul, "length", "area"),
        (
            "thermal_conductance",
            ProductOp::Mul,
            "duration",
            "heat_capacity",
        ),
        (
            "duration",
            ProductOp::Mul,
            "thermal_conductance",
            "heat_capacity",
        ),
        ("energy", ProductOp::Div, "mass", "specific_energy"),
        ("energy", ProductOp::Div, "specific_energy", "mass"),
        (
            "energy",
            ProductOp::Div,
            "heat_capacity",
            "temperature_delta",
        ),
        (
            "energy",
            ProductOp::Div,
            "temperature_delta",
            "heat_capacity",
        ),
        ("heat_capacity", ProductOp::Div, "mass", "specific_heat"),
        ("heat_capacity", ProductOp::Div, "specific_heat", "mass"),
        (
            "heat_capacity",
            ProductOp::Div,
            "duration",
            "thermal_conductance",
        ),
        (
            "heat_capacity",
            ProductOp::Div,
            "thermal_conductance",
            "duration",
        ),
    ];
    #[test]
    fn thermal_products_are_derived() {
        let r = registry();
        let kind = |id| r.kind(id).unwrap();
        for (left, op, right, result) in THERMAL_PRODUCTS {
            assert_eq!(
                kind(left).product(op, kind(right)),
                Ok(kind(result)),
                "{left} {op} {right}"
            );
        }
        let catalog: Catalog =
            serde_yaml::from_str(include_str!("../../../profiles/thermal.yml")).unwrap();
        assert!(catalog.operations.len() < 13);
        assert_eq!(catalog.operations.len(), 0);
        let thermal: Vec<&str> = THERMAL_PRODUCTS
            .iter()
            .flat_map(|&(left, _, right, result)| [left, right, result])
            .collect();
        for row in &catalog.operations {
            for id in [&row.left, &row.right, &row.result] {
                assert!(!thermal.contains(&id.as_str()), "{id}");
            }
        }
    }
    #[test]
    fn floors_are_declared_and_readable() {
        let r = registry();
        let kind = |id| r.kind(id).unwrap();
        assert_eq!(kind("mass").minimum(), Some(q(0.0, "kg")));
        assert_eq!(kind("temperature").minimum(), Some(q(0.0, "K")));
        for id in ["energy", "time", "duration"] {
            assert_eq!(kind(id).minimum(), None, "{id}");
        }
        assert_eq!(
            q(0.0, "K").apply(Op::Sub, q(1.0, "delta_K")),
            Err(QuantityError::BelowMinimum {
                kind: "temperature".into(),
                unit: "kelvin".into(),
                minimum: ExactScalar::zero(),
                value: ExactScalar::parse("-1").unwrap(),
            })
        );
    }
    #[test]
    fn time_is_a_point_whose_differences_are_durations() {
        let r = registry();
        let kind = |id| r.kind(id).unwrap();
        assert_eq!(r.time(), Some(kind("time")));
        let elapsed = q(3.0, "s").apply(Op::Sub, q(1.0, "s")).unwrap();
        assert_eq!(elapsed, q(2.0, "delta_s"));
        assert_eq!(elapsed.kind(), kind("duration"));
        assert!(matches!(
            q(1.0, "s").apply(Op::Add, q(1.0, "s")),
            Err(QuantityError::UnsupportedOperation { .. })
        ));
        let capacity = q(2.0, "W/K").apply(Op::Mul, q(3.0, "delta_s")).unwrap();
        assert_eq!(capacity, q(6.0, "J/K"));
        assert_eq!(capacity.kind(), kind("heat_capacity"));
        assert!(matches!(
            q(2.0, "W/K").apply(Op::Mul, q(3.0, "s")),
            Err(QuantityError::UnsupportedOperation { .. })
        ));
        let step = q(6.0, "J/K").apply(Op::Div, q(2.0, "W/K")).unwrap();
        assert_eq!(step, q(3.0, "delta_s"));
        assert_eq!(step.kind(), kind("duration"));
    }
    #[test]
    fn comparisons_tolerances_and_signs() {
        assert_eq!(q(2.0, "J").compare(q(0.001, "kJ")), Ok(Ordering::Greater));
        let residual = q(-0.5, "J");
        assert_eq!(residual.abs().unwrap(), q(0.5, "J"));
        assert_eq!(residual.within(q(0.0005, "kJ")), Ok(true));
        assert_eq!(residual.within(q(0.4, "J")), Ok(false));
        assert!(q(0.0, "J").is_zero() && !residual.is_zero());
        assert!(q(1.0, "K").abs().is_err());
    }
    #[test]
    fn numbers_stay_finite() {
        let r = registry();
        assert_eq!(
            r.quantity_for_symbol(f64::NAN, "g", None),
            Err(QuantityError::NonFiniteInput)
        );
        assert_eq!(
            q(f64::MAX, "J").scale(2.0),
            Err(QuantityError::NumericalFailure)
        );
        assert_eq!(
            q(1.0, "J").apply(Op::Div, q(0.0, "kg")),
            Err(QuantityError::DivisionByZero)
        );
        assert!(matches!(
            r.quantity_for_symbol(1.0, "guess", None),
            Err(QuantityError::Unknown { .. })
        ));
    }
    #[test]
    fn documents_read_kinds_and_quantities_against_the_profile() {
        let heat: Quantity<'static> = serde_yaml::from_str("{value: 2, unit: kJ}").unwrap();
        assert_eq!(heat, q(2000.0, "J"));
        let kind: Kind<'static> = serde_yaml::from_str("specific_heat").unwrap();
        assert_eq!(kind.id(), "specific_heat");
        assert!(
            serde_yaml::from_str::<Quantity<'static>>("{value: 1, unit: J, kind: torque}").is_err()
        );
        assert!(serde_yaml::from_str::<Kind<'static>>("SpecificHeat").is_err());
    }
}
