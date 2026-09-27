//! The bundled thermal catalog, read through the public API.
use std::cmp::Ordering;

use num_rational::BigRational;

use bridgman_core::{
    thermal, AffineRole, Catalog, DerivationError, Dimensions, ExactScalar, ExactValue, Exponent,
    Grade, Kind, Op, Operation, ProductOp, Quantity, QuantityError, Record, Term, Unit,
};

fn q(value: f64, symbol: &str) -> Quantity<'static> {
    thermal().quantity_for_symbol(value, symbol, None).unwrap()
}
fn kind(id: &str) -> Kind<'static> {
    thermal().kind(id).unwrap()
}
fn unit(id: &str) -> Unit<'static> {
    thermal().unit(id).unwrap()
}
#[track_caller]
fn same(actual: Quantity<'static>, expected: Quantity<'static>) {
    assert_eq!(
        actual.equals_exactly(expected),
        Ok(true),
        "{actual} is not {expected}"
    );
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
    same(grams, capacity);
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
    same(delta.apply(Op::Add, cold).unwrap(), warmed);
    assert!(matches!(
        cold.apply(Op::Add, hot),
        Err(QuantityError::UnsupportedOperation { .. })
    ));
    assert_eq!(
        q(0.0, "K").apply(Op::Sub, q(1.0, "delta_K")).err(),
        Some(QuantityError::BelowMinimum {
            kind: kind("temperature"),
            unit: unit("kelvin"),
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
    assert_eq!(kind("temperature").role(), AffineRole::Point);
}
#[test]
fn exact_conversion_keeps_each_kinds_affine_role() {
    let twenty = || ExactValue::from_scalar(ExactScalar::parse("20").unwrap());
    let point = kind("temperature").convert_exact(twenty(), unit("degree_celsius"), unit("kelvin"));
    let exact = ExactValue::from_scalar(ExactScalar::parse("5863/20").unwrap());
    assert_eq!(point, Ok(exact));
    let difference = kind("temperature_delta").convert_exact(
        twenty(),
        unit("degree_celsius_delta"),
        unit("kelvin_delta"),
    );
    assert_eq!(difference, Ok(twenty()));
    assert!(matches!(
        kind("energy").convert_exact(twenty(), unit("joule"), unit("newton_metre")),
        Err(QuantityError::UnitKindMismatch { .. })
    ));
}
#[test]
fn affine_overflow_is_an_error() {
    let hot = q(1e308, "K");
    assert_eq!(
        hot.apply(Op::Add, q(1e308, "delta_K")).err(),
        Some(QuantityError::NumericalFailure)
    );
    assert_eq!(
        q(1e308, "delta_K")
            .apply(Op::Sub, q(-1e308, "delta_K"))
            .err(),
        Some(QuantityError::NumericalFailure)
    );
}
#[test]
fn equal_dimensions_do_not_make_kinds_interchangeable() {
    let (energy, torque) = (kind("energy"), kind("torque"));
    assert_eq!(energy.dimensions(), torque.dimensions());
    assert_eq!(
        q(1.0, "J").apply(Op::Add, q(1.0, "N*m")).err(),
        Some(QuantityError::KindMismatch {
            expected: energy,
            actual: torque,
        })
    );
    assert!(q(1.0, "J").compare(q(1.0, "N*m")).is_err());
    assert_eq!(kind("unitless").dimensions().unwrap(), &Dimensions::one());
}
#[test]
fn the_dimensionless_kind_scales_and_cancels() {
    let heat = q(3.0, "J");
    let ratio = heat.apply(Op::Div, q(1.5, "J")).unwrap();
    assert_eq!(ratio.kind().id(), "unitless");
    assert_eq!(ratio.in_symbol("1").unwrap(), 2.0);
    same(heat.apply(Op::Mul, ratio).unwrap(), q(6.0, "J"));
    assert!(matches!(
        q(1.0, "J").apply(Op::Mul, q(1.0, "J")),
        Err(QuantityError::NoProductKind { .. })
    ));
}
/// Every product the thermal catalog once declared as a row.
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
    for (left, op, right, result) in THERMAL_PRODUCTS {
        assert_eq!(
            kind(left).product(op, kind(right)),
            Ok(kind(result)),
            "{left} {op} {right}"
        );
    }
    let catalog: Catalog =
        serde_yaml::from_str(include_str!("../../../catalogs/thermal.yml")).unwrap();
    assert_eq!(catalog.operations.len(), 0);
}
fn exponent(numer: i64, denom: i64) -> BigRational {
    BigRational::new(numer.into(), denom.into())
}
fn exact(numer: i64, denom: i64) -> Exponent {
    Exponent::Exact(exponent(numer, denom))
}
#[test]
fn powers_derive_from_dimensions_and_grade() {
    let power = |id, n| kind(id).power(&exact(n, 1));
    assert_eq!(power("length", 2), Ok(kind("area")));
    assert_eq!(power("frequency", -1), Ok(kind("duration")));
    assert_eq!(power("unitless", 3), Ok(kind("unitless")));
    assert_eq!(power("energy", 0), Ok(kind("unitless")));
    assert_eq!(power("velocity", 1), Ok(kind("velocity")));
    assert_eq!(
        power("velocity", 2),
        Err(QuantityError::UngradedPower {
            base: kind("velocity"),
            exponent: exponent(2, 1),
            grade: Grade::Vector,
        })
    );
    assert_eq!(
        power("area", -1),
        Err(QuantityError::NoPowerKind {
            base: kind("area"),
            exponent: exponent(-1, 1),
            dimensions: Dimensions::from_integer_powers([("L", -2)]),
            grade: Grade::Scalar,
        })
    );
    assert_eq!(
        power("temperature", 2),
        Err(QuantityError::UnsupportedOperation {
            operation: Operation::Power(exact(2, 1)),
            left: kind("temperature"),
            right: None,
        })
    );
    assert_eq!(
        kind("unitless").power(&Exponent::Inexact),
        Err(QuantityError::UnsupportedOperation {
            operation: Operation::Power(Exponent::Inexact),
            left: kind("unitless"),
            right: None,
        })
    );
}
#[test]
fn a_root_is_a_rational_power() {
    let half = exponent(1, 2);
    assert_eq!(kind("area").power(&exact(1, 2)), Ok(kind("length")));
    assert_eq!(
        kind("length").power(&exact(1, 2)),
        Err(QuantityError::NoPowerKind {
            base: kind("length"),
            exponent: half.clone(),
            dimensions: Dimensions::from_rational_powers([("L", half.clone())]),
            grade: Grade::Scalar,
        })
    );
}
#[test]
fn a_square_is_the_product_with_itself() {
    for k in thermal().kinds() {
        assert_eq!(
            k.power(&exact(2, 1)).ok(),
            k.product(ProductOp::Mul, k).ok(),
            "{k}"
        );
    }
}
#[test]
fn a_pure_number_is_a_term_without_a_kind() {
    let (energy, temperature) = (Term::Kind(kind("energy")), Term::Kind(kind("temperature")));
    assert_eq!(Term::Number.combine(Op::Mul, energy), Ok(energy));
    assert_eq!(energy.combine(Op::Div, Term::Number), Ok(energy));
    assert_eq!(
        Term::Number.combine(Op::Add, Term::Number),
        Ok(Term::Number)
    );
    assert_eq!(
        Term::Number.combine(Op::Div, Term::Kind(kind("duration"))),
        Ok(Term::Kind(kind("frequency")))
    );
    assert_eq!(
        energy.combine(Op::Add, Term::Number),
        Err(QuantityError::NumberTerm {
            operation: Operation::Binary(Op::Add),
            kind: kind("energy"),
        })
    );
    assert_eq!(
        Term::Number.combine(Op::Mul, temperature),
        Err(QuantityError::UnsupportedOperation {
            operation: Operation::Scale,
            left: kind("temperature"),
            right: None,
        })
    );
    assert_eq!(
        energy.same(Term::Number),
        Err(QuantityError::NumberTerm {
            operation: Operation::Compare,
            kind: kind("energy"),
        })
    );
    assert_eq!(
        energy.same(Term::Kind(kind("torque"))),
        Err(QuantityError::KindMismatch {
            expected: kind("energy"),
            actual: kind("torque"),
        })
    );
    assert_eq!(Term::Number.same(Term::Number), Ok(Term::Number));
    assert_eq!(Term::Number.power(&Exponent::Inexact), Ok(Term::Number));
    assert_eq!(
        Term::Kind(kind("area")).power(&exact(1, 2)),
        Ok(Term::Kind(kind("length")))
    );
    assert_eq!(energy.absolute(), Ok(energy));
    assert!(temperature.absolute().is_err());
}
#[test]
fn a_pure_number_scales_every_kind_but_a_point() {
    assert_eq!(
        kind("energy").scaled(Operation::DivideScalar),
        Ok(kind("energy"))
    );
    assert_eq!(
        kind("temperature").scaled(Operation::Scale),
        Err(QuantityError::UnsupportedOperation {
            operation: Operation::Scale,
            left: kind("temperature"),
            right: None,
        })
    );
}
#[test]
fn a_refusal_reports_its_variant_and_fields_by_id() {
    let refused = kind("energy").same(kind("torque")).unwrap_err();
    let name: &'static str = (&refused).into();
    assert_eq!(name, "KindMismatch");
    assert!(<QuantityError as strum::VariantNames>::VARIANTS.contains(&name));
    assert_eq!(
        serde_json::to_value(&refused).unwrap(),
        serde_json::json!({
            "variant": "KindMismatch",
            "fields": {"expected": "energy", "actual": "torque"}
        })
    );
    let root = kind("length").power(&exact(1, 2)).unwrap_err();
    assert_eq!(
        serde_json::to_value(&root).unwrap()["fields"]["exponent"],
        "1/2"
    );
}
#[test]
fn values_are_compared_only_within_one_kind() {
    assert_eq!(kind("energy").same(kind("energy")), Ok(kind("energy")));
    assert_eq!(
        kind("energy").same(kind("torque")),
        Err(QuantityError::KindMismatch {
            expected: kind("energy"),
            actual: kind("torque"),
        })
    );
}
#[test]
fn floors_are_declared_and_readable() {
    same(kind("mass").minimum().unwrap(), q(0.0, "kg"));
    same(kind("temperature").minimum().unwrap(), q(0.0, "K"));
    for id in ["energy", "momentum", "time", "duration", "enthalpy"] {
        assert!(kind(id).minimum().is_none(), "{id}");
    }
}
#[test]
fn time_is_a_point_whose_differences_are_durations() {
    assert_eq!(thermal().time(), Some(kind("time")));
    let elapsed = q(3.0, "s").apply(Op::Sub, q(1.0, "s")).unwrap();
    same(elapsed, q(2.0, "delta_s"));
    assert_eq!(elapsed.kind(), kind("duration"));
    assert_eq!(kind("time").difference(), kind("duration"));
    assert!(matches!(
        q(1.0, "s").apply(Op::Add, q(1.0, "s")),
        Err(QuantityError::UnsupportedOperation { .. })
    ));
    let capacity = q(2.0, "W/K").apply(Op::Mul, q(3.0, "delta_s")).unwrap();
    same(capacity, q(6.0, "J/K"));
    assert_eq!(capacity.kind(), kind("heat_capacity"));
    assert_eq!(
        q(2.0, "W/K").apply(Op::Mul, q(3.0, "s")).err(),
        Some(QuantityError::Derivation(DerivationError::Point {
            left: kind("thermal_conductance"),
            op: ProductOp::Mul,
            right: kind("time"),
            point: kind("time"),
        }))
    );
    let step = q(6.0, "J/K").apply(Op::Div, q(2.0, "W/K")).unwrap();
    same(step, q(3.0, "delta_s"));
    assert_eq!(step.kind(), kind("duration"));
}
#[test]
fn enthalpy_is_a_point_whose_differences_are_energy() {
    let change = q(10.0, "enthalpy_kJ")
        .apply(Op::Sub, q(4000.0, "enthalpy_J"))
        .unwrap();
    same(change, q(6000.0, "J"));
    let raised = q(1.0, "enthalpy_J").apply(Op::Add, q(1.0, "J")).unwrap();
    assert_eq!(raised.kind(), kind("enthalpy"));
    assert!(matches!(
        q(1.0, "enthalpy_J").apply(Op::Add, q(1.0, "enthalpy_J")),
        Err(QuantityError::UnsupportedOperation { .. })
    ));
    assert!(matches!(
        q(1.0, "enthalpy_J").apply(Op::Mul, q(1.0, "kg")),
        Err(QuantityError::Derivation(DerivationError::Point { .. }))
    ));
    assert_eq!(kind("energy").role(), AffineRole::Difference);
    same(q(1.0, "J").scale(2.0).unwrap(), q(2.0, "J"));
}
#[test]
fn force_dot_displacement_is_energy_and_wedge_is_torque() {
    let (force, displacement) = (kind("force"), kind("displacement"));
    assert_eq!(
        force.product(ProductOp::Dot, displacement),
        Ok(kind("energy"))
    );
    assert_eq!(
        force.product(ProductOp::Wedge, displacement),
        Ok(kind("torque"))
    );
    assert_eq!(kind("torque").grade(), Grade::Bivector);
    same(
        q(3.0, "N").apply(Op::Dot, q(2.0, "vec_m")).unwrap(),
        q(6.0, "J"),
    );
}
#[test]
fn two_vectors_need_dot_or_wedge() {
    assert_eq!(
        kind("force").product(ProductOp::Mul, kind("displacement")),
        Err(QuantityError::Derivation(DerivationError::Ungraded {
            left: kind("force"),
            op: ProductOp::Mul,
            right: kind("displacement"),
            left_grade: Grade::Vector,
            right_grade: Grade::Vector,
        }))
    );
    assert!(matches!(
        kind("mass").product(ProductOp::Dot, kind("velocity")),
        Err(QuantityError::Derivation(DerivationError::Ungraded { .. }))
    ));
}
#[test]
fn frequency_and_angular_velocity_do_not_add() {
    assert_eq!(
        q(1.0, "Hz").apply(Op::Add, q(1.0, "rad/s")).err(),
        Some(QuantityError::KindMismatch {
            expected: kind("frequency"),
            actual: kind("angular_velocity"),
        })
    );
    assert_eq!(
        kind("angle").product(ProductOp::Div, kind("duration")),
        Ok(kind("angular_velocity"))
    );
    assert_eq!(
        kind("unitless").product(ProductOp::Div, kind("duration")),
        Ok(kind("frequency"))
    );
}
#[test]
fn angle_is_a_dimension_one_kind() {
    let angle = kind("angle");
    assert_eq!(angle.dimensions(), Ok(&Dimensions::one()));
    assert_eq!(angle.grade(), Grade::Bivector);
    assert_ne!(angle, kind("unitless"));
    assert!(matches!(
        q(1.0, "rad").apply(Op::Add, q(1.0, "1")),
        Err(QuantityError::KindMismatch { .. })
    ));
}
#[test]
fn a_rate_times_a_duration_is_what_it_is_the_rate_of() {
    assert_eq!(kind("force").rate_of(), Some(kind("momentum")));
    assert_eq!(kind("momentum").rate(), Some(kind("force")));
    assert_eq!(kind("energy").rate(), Some(kind("power")));
    let momentum = q(2.0, "N").apply(Op::Mul, q(3.0, "delta_s")).unwrap();
    same(momentum, q(6.0, "kg*m/s"));
    assert_eq!(momentum.kind(), kind("momentum"));
    let power = q(6.0, "J").apply(Op::Div, q(2.0, "delta_s")).unwrap();
    assert_eq!(power.kind(), kind("power"));
}
#[test]
fn comparisons_tolerances_and_signs() {
    assert_eq!(q(2.0, "J").compare(q(0.001, "kJ")), Ok(Ordering::Greater));
    let residual = q(-0.5, "J");
    same(residual.abs().unwrap(), q(0.5, "J"));
    assert_eq!(residual.within(q(0.0005, "kJ")), Ok(true));
    assert_eq!(residual.within(q(0.4, "J")), Ok(false));
    assert!(q(0.0, "J").is_zero() && !residual.is_zero());
    assert!(q(1.0, "K").abs().is_err());
    same(q(1000.0, "g"), q(1.0, "kg"));
}
#[test]
fn numbers_stay_finite() {
    let r = thermal();
    assert_eq!(
        r.quantity_for_symbol(f64::NAN, "g", None).err(),
        Some(QuantityError::NonFiniteInput)
    );
    assert_eq!(
        q(f64::MAX, "J").scale(2.0).err(),
        Some(QuantityError::NumericalFailure)
    );
    assert_eq!(
        q(1.0, "J").apply(Op::Div, q(0.0, "kg")).err(),
        Some(QuantityError::DivisionByZero)
    );
    assert_eq!(
        r.quantity_for_symbol(1.0, "guess", None).err(),
        Some(QuantityError::Derivation(DerivationError::Unknown {
            record: Record::UnitSymbol,
            id: "guess".into(),
        }))
    );
}
