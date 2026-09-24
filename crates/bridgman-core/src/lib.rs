//! Python-independent quantity semantics for Bridgman.
//!
//! There is one engine. A `Catalog` declares kinds, units and twin rows as
//! data; `Registry::compile` checks it and hands out `Kind` and `Unit` handles;
//! a `Quantity` is a finite value of a kind, and every operation on it asks
//! `Kind::combine` which kind results. `profile` is the catalog Bridgman
//! bundles, read from `profiles/thermal.yml` like any other.

pub mod profile;

mod catalog;
mod compile;
mod derive;
mod dimension;
mod error;
mod grade;
mod pi;
mod quantity;
mod qudv;
mod registry;
mod scalar;

pub use catalog::{
    Catalog, Conversion, KindDecl, Magnitude, Op, OperationDecl, OperationParseError, ProductOp,
    UnitDecl, CATALOG_SCHEMA,
};
pub use dimension::{DimensionError, Dimensions};
pub use error::{CatalogError, Operation, QuantityError, RateFault, Record, Shared};
pub use grade::{Grade, GradeError};
pub use pi::{count_pi_groups_exact, pi_groups_exact};
pub use quantity::Quantity;
pub use qudv::qudv_schema2_to_catalog;
pub use registry::{AffineRole, Kind, Registry, Unit};
pub use scalar::{ExactScalar, ExactValue, ScalarError};
