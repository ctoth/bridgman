use std::collections::BTreeMap;

use bridgman_core::profile::{JOULE_PER_KG_K, KELVIN_DELTA, KILOGRAM, KILOJOULE};
use bridgman_core::{
    AffineRole, Catalog, Conversion, Dimensions, ExactScalar, ExactValue, KindDecl, Magnitude,
    OperationDecl, ProductOp, QuantityError, Registry, UnitDecl, CATALOG_SCHEMA,
};
use num_bigint::BigInt;
use serde_yaml::Value;

fn scalar(value: &str) -> ExactScalar {
    ExactScalar::parse(value).unwrap()
}

fn unit(
    id: &str,
    symbol: &str,
    kinds: &[&str],
    reference: &str,
    scale: &str,
    offset: &str,
) -> UnitDecl {
    let coherent_scale = match id {
        "gram" => "1/1000",
        "kilogram" => "1",
        "mass_squared" => "1/1000000",
        _ => scale,
    };
    UnitDecl {
        id: id.into(),
        symbol: symbol.into(),
        kinds: kinds.iter().map(|kind| (*kind).into()).collect(),
        conversion: Some(Conversion {
            reference_unit: reference.into(),
            scale: Magnitude::Exact(scalar(scale)),
            offset: Magnitude::Exact(ExactValue::from_scalar(scalar(offset))),
        }),
        coherent_scale: Some(scalar(coherent_scale)),
    }
}

fn contract_registry() -> Registry {
    let temperature = Dimensions::from_integer_powers([("Theta", 1)]);
    let energy = Dimensions::from_integer_powers([("M", 1), ("L", 2), ("T", -2)]);
    Registry::compile(Catalog {
        schema: CATALOG_SCHEMA,
        provenance: BTreeMap::new(),
        kinds: vec![
            KindDecl {
                id: "temperature".into(),
                dimensions: Some(temperature.clone()),
                difference_kind: Some("temperature_difference".into()),
            },
            KindDecl {
                id: "temperature_difference".into(),
                dimensions: Some(temperature),
                difference_kind: None,
            },
            KindDecl {
                id: "mass".into(),
                dimensions: Some(Dimensions::from_integer_powers([("M", 1)])),
                difference_kind: None,
            },
            KindDecl {
                id: "mass_squared".into(),
                dimensions: Some(Dimensions::from_integer_powers([("M", 2)])),
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
                id: "angle".into(),
                dimensions: Some(Dimensions::one()),
                difference_kind: None,
            },
            KindDecl {
                id: "widget_count".into(),
                dimensions: Some(Dimensions::one()),
                difference_kind: None,
            },
            KindDecl {
                id: "generalized_coordinate".into(),
                dimensions: None,
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
                "Celsius",
                &["temperature", "temperature_difference"],
                "kelvin",
                "1",
                "27315/100",
            ),
            unit(
                "milli_celsius",
                "MilliCelsius",
                &["temperature", "temperature_difference"],
                "kelvin",
                "1/1000",
                "27315/100",
            ),
            unit("gram", "Gram", &["mass"], "gram", "1", "0"),
            unit("kilogram", "Kilogram", &["mass"], "gram", "1000", "0"),
            unit(
                "mass_squared",
                "GramSquared",
                &["mass_squared"],
                "mass_squared",
                "1",
                "0",
            ),
            unit("joule", "Joule", &["energy"], "joule", "1", "0"),
            unit("joule_product", "N*m", &["energy"], "joule", "1", "0"),
            unit("newton_metre", "N*m", &["torque"], "newton_metre", "1", "0"),
            unit("radian", "Radian", &["angle"], "radian", "1", "0"),
            unit(
                "degree",
                "DegreeAngle",
                &["angle"],
                "radian",
                "1/180*pi^1",
                "0",
            ),
            unit("widget", "widget", &["widget_count"], "widget", "1", "0"),
            unit(
                "generalized",
                "generalized",
                &["generalized_coordinate"],
                "generalized",
                "1",
                "0",
            ),
        ],
        operations: vec![OperationDecl {
            left: "mass".into(),
            op: ProductOp::Mul,
            right: "mass".into(),
            result: "mass_squared".into(),
            commutative: false,
            provenance: Some("quantity-contract-cases.yml".into()),
        }],
    })
    .unwrap()
}

fn number(value: &Value) -> f64 {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|v| v as f64))
        .unwrap()
}

fn expected_number(value: &Value) -> f64 {
    if let Some(value) = value.as_i64() {
        value as f64
    } else if let Some(value) = value.as_f64() {
        value
    } else {
        scalar(value.as_str().unwrap()).to_f64().unwrap()
    }
}

#[test]
fn quantity_contract_cases_execute_their_declared_examples() {
    let document: Value =
        serde_yaml::from_str(include_str!("../../../design/quantity-contract-cases.yml")).unwrap();
    assert_eq!(document["status"], "executable");
    let cases = document["cases"].as_sequence().unwrap();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        let registry = contract_registry();
        match id {
            "celsius_point" | "celsius_difference" | "milli_celsius_point" => {
                let given = &case["given"];
                let (kind, role) = match given["role"].as_str().unwrap() {
                    "point" => ("temperature", AffineRole::Point),
                    "difference" => ("temperature_difference", AffineRole::Difference),
                    other => panic!("unknown role {other}"),
                };
                let kind = registry.kind(kind).unwrap();
                assert_eq!(registry.role(kind), Ok(role), "{id}");
                let quantity = registry
                    .quantity_for_symbol(
                        number(&given["value"]),
                        given["unit"].as_str().unwrap(),
                        Some(kind),
                    )
                    .unwrap();
                let actual = quantity
                    .in_unit(&registry, registry.unit("kelvin").unwrap())
                    .unwrap();
                assert!(
                    (actual - expected_number(&case["expected"]["value"])).abs() < 1e-12,
                    "{id}"
                );
                assert_eq!(
                    case["expected"]["role"].as_str().unwrap(),
                    given["role"].as_str().unwrap()
                );
            }
            "point_subtraction" => {
                let kind = registry.kind("temperature").unwrap();
                let unit = registry.unit("celsius").unwrap();
                let result = registry
                    .quantity(30.0, unit, kind)
                    .unwrap()
                    .sub(&registry, registry.quantity(20.0, unit, kind).unwrap())
                    .unwrap();
                assert_eq!(registry.role(result.kind()), Ok(AffineRole::Difference));
                assert_eq!(
                    result
                        .in_unit(&registry, registry.unit("kelvin").unwrap())
                        .unwrap(),
                    number(&case["expected"]["value"])
                );
            }
            "point_addition" => {
                let kind = registry.kind("temperature").unwrap();
                let unit = registry.unit("celsius").unwrap();
                let a = registry.quantity(30.0, unit, kind).unwrap();
                let b = registry.quantity(20.0, unit, kind).unwrap();
                assert_eq!(
                    a.add(&registry, b),
                    Err(QuantityError::UnsupportedAffineOperation)
                );
                assert_eq!(case["expected_error"], "unsupported_affine_operation");
            }
            "reference_is_not_si" => {
                let q = registry
                    .quantity_for_symbol(
                        number(&case["given"]["value"]),
                        "Kilogram",
                        Some(registry.kind("mass").unwrap()),
                    )
                    .unwrap();
                assert_eq!(
                    q.in_unit(&registry, registry.unit("gram").unwrap())
                        .unwrap(),
                    number(&case["expected"]["value"])
                );
            }
            "exact_angle" => {
                let converted = registry
                    .convert_exact(
                        ExactValue::from_scalar(scalar("180")),
                        registry.unit("degree").unwrap(),
                        registry.unit("radian").unwrap(),
                        registry.kind("angle").unwrap(),
                    )
                    .unwrap();
                let terms: Vec<_> = converted.terms().collect();
                assert_eq!(
                    terms[0].rational.to_string(),
                    case["expected"]["rational"].as_str().unwrap()
                );
                assert_eq!(
                    terms[0].pi_exponent,
                    BigInt::from(case["expected"]["pi_exponent"].as_i64().unwrap())
                );
            }
            "semantic_twins" => {
                let energy = registry
                    .quantity_for_symbol(1.0, "Joule", Some(registry.kind("energy").unwrap()))
                    .unwrap();
                let torque = registry
                    .quantity_for_symbol(1.0, "N*m", Some(registry.kind("torque").unwrap()))
                    .unwrap();
                assert!(matches!(
                    energy.add(&registry, torque),
                    Err(QuantityError::KindMismatch { .. })
                ));
                assert_eq!(case["expected_error"], "kind_mismatch");
            }
            "ambiguous_unit" => {
                assert_eq!(
                    registry.quantity_for_symbol(1.0, "N*m", None),
                    Err(QuantityError::AmbiguousKind("N*m".into()))
                );
                assert_eq!(case["expected_error"], "ambiguous_kind");
            }
            "extension" => {
                let q = registry
                    .quantity_for_symbol(
                        1.0,
                        "widget",
                        Some(registry.kind("widget_count").unwrap()),
                    )
                    .unwrap();
                assert_eq!(
                    q.in_unit(&registry, registry.unit("widget").unwrap())
                        .unwrap(),
                    1.0
                );
                assert_eq!(
                    case["expected"],
                    "numeric_construction_without_rust_changes"
                );
            }
            "unknown_dimensions" => {
                assert_eq!(
                    registry.quantity(
                        1.0,
                        registry.unit("generalized").unwrap(),
                        registry.kind("generalized_coordinate").unwrap(),
                    ),
                    Err(QuantityError::UnresolvedDimensions(
                        "generalized_coordinate".into()
                    ))
                );
                assert_eq!(case["expected_error"], "unresolved_dimensions");
            }
            "cross_registry_handle" => {
                let other = contract_registry();
                assert_eq!(
                    registry.dimensions(other.kind("mass").unwrap()),
                    Err(QuantityError::RegistryMismatch)
                );
                assert_eq!(case["expected_error"], "registry_mismatch");
            }
            "arbitrary_precision" => {
                let scale = case["scale"].as_str().unwrap();
                let value = scalar(scale);
                assert_eq!(ExactScalar::parse(&value.encoded()).unwrap(), value);
                assert_eq!(case["expected"], "exact_round_trip");
            }
            "finite_input_overflow" => {
                let mass = registry.kind("mass").unwrap();
                let a = registry
                    .quantity(1e308, registry.unit("gram").unwrap(), mass)
                    .unwrap();
                assert_eq!(a.mul(&registry, a), Err(QuantityError::NumericalFailure));
                assert_eq!(case["expected_error"], "numerical_failure");
            }
            "explicit_rules_only" => {
                let energy = registry
                    .quantity(
                        1.0,
                        registry.unit("joule").unwrap(),
                        registry.kind("energy").unwrap(),
                    )
                    .unwrap();
                assert!(matches!(
                    energy.mul(&registry, energy),
                    Err(QuantityError::MissingOperationRule { .. })
                ));
                assert_eq!(case["expected_error"], "missing_operation_rule");
            }
            "heating" => {
                let capacity = (KILOGRAM.quantity(2.0).unwrap()
                    * JOULE_PER_KG_K.quantity(500.0).unwrap())
                .unwrap();
                let heat = (capacity * KELVIN_DELTA.quantity(100.0).unwrap()).unwrap();
                assert_eq!(heat.in_unit(KILOJOULE).unwrap(), 100.0);
                assert_eq!(case["expected"]["energy"], "100 kJ");
            }
            "legacy_root_contract" | "legacy_root_success" => {
                let python = include_str!("../../../tests/test_symbolic.py");
                assert!(python.contains("def test_legacy_root_contract_cases"));
                assert_eq!(
                    case["python_test"],
                    "tests/test_symbolic.py::test_legacy_root_contract_cases"
                );
            }
            unknown => panic!("unhandled quantity contract case {unknown}"),
        }
    }
}
