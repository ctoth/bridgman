use bridgman_core::{profile::registry, Op, Quantity};

fn q(kind: &str, values: &[f64]) -> Quantity<'static> {
    let kind = registry().kind(kind).unwrap();
    Quantity::from_components(values, kind.canonical_unit().unwrap(), kind).unwrap()
}

fn components(value: Quantity<'_>) -> Vec<f64> {
    value
        .components_in(value.kind().canonical_unit().unwrap())
        .unwrap()
}

#[test]
fn off_centre_impulse_uses_vector_and_bivector_coordinates() {
    let r = q("displacement", &[2.0, 0.0, 0.0]);
    let impulse = q("momentum", &[0.0, 3.0, 0.0]);
    let dv = impulse.apply(Op::Div, q("mass", &[10.0])).unwrap();
    assert_eq!(dv.kind(), registry().kind("velocity").unwrap());
    assert_eq!(components(dv), [0.0, 0.3, 0.0]);
    let angular = r.apply(Op::Wedge, impulse).unwrap();
    assert_eq!(angular.kind(), registry().kind("angular_momentum").unwrap());
    assert_eq!(components(angular), [6.0, 0.0, 0.0]);
    assert_eq!(
        components(impulse.apply(Op::Wedge, r).unwrap()),
        [-6.0, 0.0, 0.0]
    );
    assert!(r
        .apply(Op::Wedge, r)
        .unwrap_err()
        .to_string()
        .contains("kind"));
    let kinetic = impulse
        .apply(Op::Dot, dv)
        .unwrap()
        .divide_scalar(2.0)
        .unwrap();
    assert!(
        (kinetic
            .in_unit(kinetic.kind().canonical_unit().unwrap())
            .unwrap()
            - 0.45)
            .abs()
            < 1e-15
    );
}

#[test]
fn dot_product_and_norm_tolerance_do_not_use_componentwise_boxes() {
    let a = q("displacement", &[3.0, 4.0, 0.0]);
    let b = q("force", &[-4.0, 3.0, 0.0]);
    assert!(a.apply(Op::Dot, b).unwrap().is_zero());
    assert!(a.within(q("displacement", &[5.0, 0.0, 0.0])).unwrap());
    assert!(!a.within(q("displacement", &[4.0, 0.0, 0.0])).unwrap());
    assert!(a.compare(a).is_err());
    assert!(a.in_unit(a.kind().canonical_unit().unwrap()).is_err());
    assert!(!q("displacement", &[f64::MAX, f64::MAX, f64::MAX])
        .within(q("displacement", &[f64::MAX, f64::MAX, 0.0]))
        .unwrap());
}

#[test]
fn documents_validate_coordinate_shape_at_the_boundary() {
    let value: Quantity<'static> =
        serde_yaml::from_str("{value: [0, 3, 0], unit: 'kg*m/s'}").unwrap();
    assert_eq!(components(value), [0.0, 3.0, 0.0]);
    for yaml in [
        "{value: 3, unit: 'kg*m/s'}",
        "{value: [1, 2], unit: 'kg*m/s'}",
        "{value: [1, .nan, 3], unit: 'kg*m/s'}",
    ] {
        assert!(serde_yaml::from_str::<Quantity<'static>>(yaml).is_err());
    }
}

#[test]
fn induced_maps_transform_bivectors_with_orientation() {
    let r = q("displacement", &[2.0, 0.0, 0.0]);
    let impulse = q("momentum", &[0.0, 3.0, 0.0]);
    let reflection = [[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let angular = r.apply(Op::Wedge, impulse).unwrap();
    let reflected = angular.map_linear(reflection).unwrap();
    assert_eq!(components(reflected), [-6.0, 0.0, 0.0]);
    assert_eq!(
        reflected,
        r.map_linear(reflection)
            .unwrap()
            .apply(Op::Wedge, impulse.map_linear(reflection).unwrap())
            .unwrap()
    );
    let rotation = [[0.6, -0.8, 0.0], [0.8, 0.6, 0.0], [0.0, 0.0, 1.0]];
    assert_eq!(components(r.map_linear(rotation).unwrap()), [1.2, 1.6, 0.0]);
    let rotated = r
        .map_linear(rotation)
        .unwrap()
        .apply(Op::Wedge, impulse.map_linear(rotation).unwrap())
        .unwrap();
    assert!(rotated
        .apply(Op::Sub, angular.map_linear(rotation).unwrap())
        .unwrap()
        .within(q("angular_momentum", &[1e-14, 0.0, 0.0]))
        .unwrap());
}
