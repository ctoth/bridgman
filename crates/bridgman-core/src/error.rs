use std::fmt;
use std::sync::Arc;

use thiserror::Error;

use crate::{Dimensions, ExactScalar, Grade, Op, ProductOp};

/// Any quantity operation, as named when it is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Binary(Op),
    Scale,
    DivideScalar,
    Abs,
}
impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Binary(op) => op.fmt(f),
            Self::Scale => f.write_str("scaling"),
            Self::DivideScalar => f.write_str("division by a number"),
            Self::Abs => f.write_str("absolute value"),
        }
    }
}

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

/// Why a catalog could not be read, imported or compiled. These are faults of
/// the declarations; nothing about a quantity has been computed yet.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum CatalogError {
    #[error("catalog schema {actual} is unsupported; expected {expected}")]
    Schema { expected: u32, actual: u32 },
    #[error("catalog document is not valid JSON")]
    Json(#[source] Shared<serde_json::Error>),
    #[error("catalog document is not valid YAML")]
    Yaml(#[source] Shared<serde_yaml::Error>),
    #[error("duplicate {record} id {id:?}")]
    Duplicate { record: Record, id: String },
    #[error("unknown {record} id {id:?}")]
    Unknown { record: Record, id: String },
    #[error("{record} id is empty")]
    EmptyId { record: Record },
    #[error("an operation names kind {0:?}, whose dimensions are unresolved")]
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
    #[error("kind {kind:?} declares a minimum that is not finite or not in the canonical unit of every unit")]
    InvalidMinimum { kind: String },
    #[error("dimensionless kind {0:?} must be linear and of dimension one")]
    InvalidDimensionlessKind(String),
    #[error("conflicting operation rules for {left:?} {op} {right:?}")]
    ConflictingOperationRule {
        left: String,
        op: ProductOp,
        right: String,
    },
    #[error("{left:?} {op} {right:?} has dimensions {dimensions} at grade {grade}, which result {result:?} does not")]
    InvalidOperationRule {
        left: String,
        op: ProductOp,
        right: String,
        result: String,
        dimensions: Dimensions,
        grade: Grade,
    },
    #[error("{left:?} {op} {right:?} is derived as {derived:?}; a declared row (here {result:?}) is kept only for twins")]
    DerivedOperationRule {
        left: String,
        op: ProductOp,
        right: String,
        result: String,
        derived: String,
    },
    #[error("division {left:?} div {right:?} cannot be commutative")]
    CommutativeQuotient { left: String, right: String },
    #[error(
        "{left:?} {op} {right:?} has no single grade in G3 (grades {left_grade} and {right_grade})"
    )]
    UngradedOperationRule {
        left: String,
        op: ProductOp,
        right: String,
        left_grade: Grade,
        right_grade: Grade,
    },
    #[error("{left:?} {op} {right:?} names point kind {point:?}, which takes no part in products")]
    PointOperationRule {
        left: String,
        op: ProductOp,
        right: String,
        point: String,
    },
    #[error("time kind {0:?} must be a scalar point kind with a difference kind")]
    InvalidTimeKind(String),
    #[error("kind {rate:?} cannot be the rate of {of:?}: {fault}")]
    InvalidRate {
        rate: String,
        of: String,
        fault: RateFault,
    },
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

/// Why a kind's `rate_of` declaration is refused.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RateFault {
    #[error("the catalog declares no time kind")]
    NoTimeKind,
    #[error("{0:?} is a point kind")]
    PointKind(String),
    #[error("a rate times a duration has dimensions {dimensions} at grade {grade}")]
    Mismatch {
        dimensions: Dimensions,
        grade: Grade,
    },
    #[error("{0:?} is already its rate")]
    AlsoRateOf(String),
}

/// Why an operation on a compiled registry's kinds, units or quantities was
/// refused.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum QuantityError {
    #[error("unknown {record} id {id:?}")]
    Unknown { record: Record, id: String },
    #[error("dimensions for kind {0:?} are unresolved")]
    UnresolvedDimensions(String),
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
    #[error("expected kind {expected:?}, received {actual:?}")]
    KindMismatch { expected: String, actual: String },
    #[error("{operation} is not defined for {left:?} and {right:?}")]
    UnsupportedOperation {
        operation: Operation,
        left: String,
        right: Option<String>,
    },
    #[error("unit {unit:?} has an offset, which linear kind {kind:?} cannot carry")]
    OffsetOnLinearKind { unit: String, kind: String },
    #[error("a quantity of kind {kind:?} is {value} {unit}, below its declared minimum {minimum} {unit}")]
    BelowMinimum {
        kind: String,
        unit: String,
        minimum: ExactScalar,
        value: ExactScalar,
    },
    #[error("unit {unit:?} is not declared for kind {kind:?}")]
    UnitKindMismatch { unit: String, kind: String },
    #[error("unit symbol {0:?} has more than one possible kind")]
    AmbiguousKind(String),
    #[error("unit symbol {0:?} identifies more than one unit for the requested kind")]
    AmbiguousUnit(String),
    #[error("no kind has dimensions {dimensions} at grade {grade} for {left:?} {op} {right:?}")]
    NoProductKind {
        left: String,
        op: ProductOp,
        right: String,
        dimensions: Dimensions,
        grade: Grade,
    },
    #[error(
        "{left:?} {op} {right:?} has no single grade in G3 (grades {left_grade} and {right_grade})"
    )]
    UngradedProduct {
        left: String,
        op: ProductOp,
        right: String,
        left_grade: Grade,
        right_grade: Grade,
    },
    #[error("{left:?} {op} {right:?} is one of the twins {twins:?}, and no row chooses")]
    UnresolvedTwin {
        left: String,
        op: ProductOp,
        right: String,
        twins: Vec<String>,
    },
    #[error("quantity input must be finite")]
    NonFiniteInput,
    #[error("quantity arithmetic or conversion produced a nonfinite value")]
    NumericalFailure,
    #[error("quantity division requires a nonzero denominator")]
    DivisionByZero,
    #[error("conversion between the requested units is disconnected")]
    DisconnectedConversion,
}
