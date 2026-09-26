use bridgman_core::{profile::registry, Op};

#[test]
fn storage_coordinates_derive_units_products_and_floors_from_the_catalog() {
    let r = registry();
    let q = |value, symbol| r.quantity_for_symbol(value, symbol, None).unwrap();
    let volume = q(2.0, "kg").apply(Op::Mul, q(0.5, "m^3/kg")).unwrap();
    assert_eq!(volume.kind().id(), "volume");
    assert_eq!(volume.in_symbol("L").unwrap(), 1000.0);
    assert_eq!(q(1000.0, "L"), volume);
    assert!(r.quantity_for_symbol(-1.0, "m^3", None).is_err());
    assert!(r.quantity_for_symbol(-1.0, "count", None).is_err());
    assert_eq!(
        q(2.0, "count").apply(Op::Mul, q(0.5, "1")).unwrap(),
        q(1.0, "count")
    );
    assert_eq!(q(1.0, "1").kind().id(), "ratio");
    assert_ne!(q(1.0, "count").kind(), q(1.0, "1").kind());
    assert!(r.kind("unitless").is_err());
    assert!(r.quantity_for_symbol(-1.0, "m^3/kg", None).is_ok());
}
