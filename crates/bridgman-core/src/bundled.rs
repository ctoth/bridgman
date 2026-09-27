//! The thermal and mechanics catalog Bridgman bundles, declared as data in
//! `catalogs/thermal.yml` and compiled once per process, so every crate that
//! reads it holds handles of the same registry.
use crate::Registry;
use std::sync::OnceLock;

/// The bundled thermal catalog.
///
/// ```
/// use bridgman_core::{thermal, Op};
/// let r = thermal();
/// let capacity = r.quantity_for_symbol(2.0, "kg", None)?
///     .apply(Op::Mul, r.quantity_for_symbol(500.0, "J/(kg*K)", None)?)?;
/// let heat = capacity.apply(Op::Mul, r.quantity_for_symbol(100.0, "delta_K", None)?)?;
/// assert_eq!(heat.in_symbol("J")?, 100000.0);
/// // Energy and torque share dimensions but are different kinds.
/// let torque = r.quantity_for_symbol(1.0, "N*m", None)?;
/// assert!(heat.apply(Op::Add, torque).is_err());
/// # Ok::<(), bridgman_core::QuantityError<'static>>(())
/// ```
pub fn thermal() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        Registry::from_yaml(include_str!("../../../catalogs/thermal.yml"))
            .expect("the bundled thermal catalog compiles")
    })
}
