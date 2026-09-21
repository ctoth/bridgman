use std::collections::BTreeSet;

#[test]
fn every_reviewed_contract_case_has_an_executable_owner() {
    let source = include_str!("../../../design/quantity-contract-cases.yml");
    let document: serde_yaml::Value = serde_yaml::from_str(source).unwrap();
    let cases = document["cases"].as_sequence().unwrap();
    let actual: BTreeSet<_> = cases
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect();
    // Native quantity tests own conversion, affine, semantic, registry, exact,
    // extension, unresolved-data, rule and overflow cases. The established
    // Python compatibility suite owns the two legacy-root cases. Physica owns
    // heating and exercises it again when consuming this crate.
    let routed: BTreeSet<_> = [
        "celsius_point",
        "celsius_difference",
        "milli_celsius_point",
        "point_subtraction",
        "point_addition",
        "reference_is_not_si",
        "exact_angle",
        "semantic_twins",
        "ambiguous_unit",
        "extension",
        "unknown_dimensions",
        "cross_registry_handle",
        "arbitrary_precision",
        "finite_input_overflow",
        "legacy_root_contract",
        "legacy_root_success",
        "explicit_rules_only",
        "heating",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        actual, routed,
        "new acceptance cases require an executable owner"
    );
}
