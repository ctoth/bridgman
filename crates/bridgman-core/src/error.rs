use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum QuantityError {
    #[error("catalog schema {actual} is unsupported; expected {expected}")]
    Schema { expected: u32, actual: u32 },
    #[error("duplicate {record} id {id:?}")]
    Duplicate { record: &'static str, id: String },
    #[error("unknown {record} id {id:?}")]
    Unknown { record: &'static str, id: String },
    #[error("dimensions for kind {0:?} are unresolved")]
    UnresolvedDimensions(String),
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
        op: &'static str,
        right: String,
    },
    #[error("conflicting operation rules for {left:?} {op} {right:?}")]
    ConflictingOperationRule {
        left: String,
        op: &'static str,
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
    #[error("invalid catalog: {0}")]
    InvalidCatalog(String),
}
