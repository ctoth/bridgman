//! Python-independent quantity semantics for Bridgman.

pub mod profile;

mod dimension;
mod error;
mod legacy;
mod quantity;
mod qudv;
mod registry;
mod scalar;

pub use dimension::{DimensionError, Dimensions};
pub use error::QuantityError;
pub use legacy::{
    canonicalize_legacy_dims, count_pi_groups_exact, legacy_dims_equal, legacy_dims_signature,
    legacy_div_dims, legacy_mul_dims, legacy_pow_dims, pi_groups_exact, LegacyDimensions,
};
pub use quantity::{DynamicQuantity, ExactValue};
pub use qudv::qudv_schema2_to_catalog;
pub use registry::{
    AffineRole, Catalog, KindDecl, KindHandle, Op, OperationDecl, Registry, UnitDecl, UnitHandle,
    CATALOG_SCHEMA,
};
pub use scalar::ExactScalar;
