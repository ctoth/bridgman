//! Python-independent quantity semantics for Bridgman.

pub mod profile;

mod dimension;
mod error;
mod pi;
mod quantity;
mod qudv;
mod registry;
mod scalar;

pub use dimension::{DimensionError, Dimensions};
pub use error::{QuantityError, Record, Shared};
pub use pi::{count_pi_groups_exact, pi_groups_exact};
pub use quantity::{DynamicQuantity, ExactValue};
pub use qudv::qudv_schema2_to_catalog;
pub use registry::{
    AffineRole, Catalog, Conversion, KindDecl, KindHandle, Magnitude, Op, OperationDecl,
    OperationParseError, ProductOp, Registry, UnitDecl, UnitHandle, CATALOG_SCHEMA,
};
pub use scalar::{ExactScalar, ScalarError};
