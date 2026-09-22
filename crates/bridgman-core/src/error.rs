use std::fmt;
use std::sync::Arc;

use thiserror::Error;

use crate::ProductOp;

/// The catalog record an identifier names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Record {
    Kind,
    Unit,
    UnitSymbol,
}
impl Record {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Kind => "kind",
            Self::Unit => "unit",
            Self::UnitSymbol => "unit symbol",
        }
    }
}
impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A document parser's own error, kept whole so `QuantityError` stays
/// cloneable. Equality is identity: two reports are equal only when they are
/// the same report.
#[derive(Debug)]
pub struct Shared<E>(Arc<E>);
impl<E> Shared<E> {
    pub fn get(&self) -> &E {
        &self.0
    }
}
impl<E> From<E> for Shared<E> {
    fn from(error: E) -> Self {
        Self(Arc::new(error))
    }
}
impl<E> Clone for Shared<E> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}
impl<E> PartialEq for Shared<E> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl<E> Eq for Shared<E> {}
impl<E: fmt::Display> fmt::Display for Shared<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<E: std::error::Error> std::error::Error for Shared<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum QuantityError {
    #[error("catalog schema {actual} is unsupported; expected {expected}")]
    Schema { expected: u32, actual: u32 },
    #[error("catalog document is not valid JSON")]
    CatalogJson(#[source] Shared<serde_json::Error>),
    #[error("duplicate {record} id {id:?}")]
    Duplicate { record: Record, id: String },
    #[error("unknown {record} id {id:?}")]
    Unknown { record: Record, id: String },
    #[error("{record} id is empty")]
    EmptyId { record: Record },
    #[error("dimensions for kind {0:?} are unresolved")]
    UnresolvedDimensions(String),
    #[error("point kind {point:?} and difference kind {difference:?} have different dimensions")]
    AffineDimensionMismatch { point: String, difference: String },
    #[error("difference kind {difference:?} of point kind {point:?} is itself a point kind")]
    NestedAffineSpace { point: String, difference: String },
    #[error("unit {unit:?} has a zero scale")]
    ZeroScale { unit: String },
    #[error("unit {unit:?} has a nonfinite approximate conversion")]
    NonFiniteConversion { unit: String },
    #[error("unit {unit:?} and its reference {reference:?} have incompatible dimensions for kind {kind:?}")]
    IncompatibleReference {
        unit: String,
        reference: String,
        kind: String,
    },
    #[error("unit {unit:?} names {reference:?}, which is not an identity terminal reference")]
    NonIdentityReference { unit: String, reference: String },
    #[error("unit {unit:?} has an unresolved conversion")]
    UnresolvedConversion { unit: String },
    #[error("unit {unit:?} has an approximate conversion")]
    ApproximateConversion { unit: String },
    #[error("unit {unit:?} has no coherent-basis scale for products")]
    MissingCoherentScale { unit: String },
    #[error("kind {kind:?} has no canonical unit")]
    NoCanonicalUnit { kind: String },
    #[error("handle belongs to another registry")]
    RegistryMismatch,
    #[error("kind mismatch: {left:?} and {right:?}")]
    KindMismatch { left: String, right: String },
    #[error("unit {unit:?} is not declared for kind {kind:?}")]
    UnitKindMismatch { unit: String, kind: String },
    #[error("unit symbol {0:?} has more than one possible kind")]
    AmbiguousKind(String),
    #[error("unit symbol {0:?} identifies more than one unit for the requested kind")]
    AmbiguousUnit(String),
    #[error("no operation rule for {left:?} {op} {right:?}")]
    MissingOperationRule {
        left: String,
        op: ProductOp,
        right: String,
    },
    #[error("conflicting operation rules for {left:?} {op} {right:?}")]
    ConflictingOperationRule {
        left: String,
        op: ProductOp,
        right: String,
    },
    #[error("operation declaration is dimensionally invalid")]
    InvalidOperationRule,
    #[error("unsupported affine operation")]
    UnsupportedAffineOperation,
    #[error("quantity input must be finite")]
    NonFiniteInput,
    #[error("quantity arithmetic or conversion produced a nonfinite value")]
    NumericalFailure,
    #[error("quantity division requires a nonzero denominator")]
    DivisionByZero,
    #[error("conversion between the requested units is disconnected")]
    DisconnectedConversion,
    #[error("QUDV document is not valid")]
    QudvDocument(#[source] Shared<serde_yaml::Error>),
    #[error("QUDV source hash is empty")]
    EmptySourceHash,
    #[error("QUDV unit {unit:?} has a non-monomial scale")]
    NonMonomialScale { unit: String },
    #[error("QUDV unit {unit:?} mixes approximate and exact terms in one sum")]
    MixedApproximateSum { unit: String },
    #[error("QUDV applied corrections cannot be recorded as JSON provenance")]
    ProvenanceEncoding(#[source] Shared<serde_json::Error>),
}
