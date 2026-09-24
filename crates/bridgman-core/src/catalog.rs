//! What a catalog declares: kinds, units and product rules, as data. Nothing
//! here is compiled in; `Registry::compile` turns a catalog into handles.
use crate::{Dimensions, ExactScalar, ExactValue, Grade};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

pub const CATALOG_SCHEMA: u32 = 4;

/// A binary operation of quantity arithmetic. `name` is its only spelling:
/// serialization, parsing and display all read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Dot,
    Wedge,
}
impl Op {
    pub(crate) const ALL: [Self; 6] = [
        Self::Add,
        Self::Sub,
        Self::Mul,
        Self::Div,
        Self::Dot,
        Self::Wedge,
    ];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Div => "div",
            Self::Dot => "dot",
            Self::Wedge => "wedge",
        }
    }
    /// The product or quotient this operation is, if it is one.
    pub const fn product(self) -> Option<ProductOp> {
        match self {
            Self::Mul => Some(ProductOp::Mul),
            Self::Div => Some(ProductOp::Div),
            Self::Dot => Some(ProductOp::Dot),
            Self::Wedge => Some(ProductOp::Wedge),
            Self::Add | Self::Sub => None,
        }
    }
}

/// The operations a kind rule may declare. Additive arithmetic is determined
/// by kind identity and declared affine spaces, not by rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProductOp {
    Mul,
    Div,
    Dot,
    Wedge,
}
impl From<ProductOp> for Op {
    fn from(op: ProductOp) -> Self {
        match op {
            ProductOp::Mul => Self::Mul,
            ProductOp::Div => Self::Div,
            ProductOp::Dot => Self::Dot,
            ProductOp::Wedge => Self::Wedge,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum OperationParseError {
    #[error("unknown operation {0:?}")]
    Unknown(String),
    #[error("operation {0} is not a product or quotient")]
    NotProduct(Op),
}
impl FromStr for Op {
    type Err = OperationParseError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|op| op.name() == text)
            .ok_or_else(|| OperationParseError::Unknown(text.into()))
    }
}
impl FromStr for ProductOp {
    type Err = OperationParseError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let op: Op = text.parse()?;
        op.product().ok_or(OperationParseError::NotProduct(op))
    }
}
impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
impl fmt::Display for ProductOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Op::from(*self).fmt(f)
    }
}
impl Serialize for Op {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}
impl<'de> Deserialize<'de> for Op {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
impl Serialize for ProductOp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Op::from(*self).serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for ProductOp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema: u32,
    #[serde(default)]
    pub provenance: BTreeMap<String, String>,
    /// The kind of pure numbers. Every other non-point kind keeps its kind
    /// when multiplied or divided by it, and a kind divided by itself is it.
    #[serde(default)]
    pub dimensionless: Option<String>,
    pub kinds: Vec<KindDecl>,
    pub units: Vec<UnitDecl>,
    #[serde(default)]
    pub operations: Vec<OperationDecl>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindDecl {
    pub id: String,
    pub dimensions: Option<Dimensions>,
    /// Grade in G3; a scalar kind need not write it.
    #[serde(default)]
    pub grade: Grade,
    #[serde(default)]
    pub difference_kind: Option<String>,
    /// The least value a quantity of this kind may take, in its canonical
    /// unit (absolute zero for thermodynamic temperature).
    #[serde(default)]
    pub minimum: Option<ExactScalar>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitDecl {
    pub id: String,
    pub symbol: String,
    pub kinds: Vec<String>,
    /// Absent while the source leaves the unit's conversion unresolved.
    #[serde(default)]
    pub conversion: Option<Conversion>,
    /// Display-unit scale in the catalog's common coherent basis, independent
    /// of its conversion reference (which may be gram rather than kilogram).
    #[serde(default)]
    pub coherent_scale: Option<ExactScalar>,
}
impl UnitDecl {
    /// A terminal reference: the unit is its own reference, reached by identity.
    pub(crate) fn is_terminal(&self) -> bool {
        self.conversion
            .as_ref()
            .is_some_and(|c| c.reference_unit == self.id && c.is_identity())
    }
}

/// `value_in_reference = scale * value + offset`; the offset applies to
/// point quantities only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conversion {
    pub reference_unit: String,
    pub scale: Magnitude<ExactScalar>,
    #[serde(default)]
    pub offset: Magnitude<ExactValue>,
}
impl Conversion {
    pub fn is_identity(&self) -> bool {
        self.scale == Magnitude::Exact(ExactScalar::one())
            && self.offset == Magnitude::Exact(ExactValue::default())
    }
}

/// A conversion magnitude: exact, or an approximation the exact-conversion API
/// refuses. On the wire an exact value is written as itself and an
/// approximation as `{"approximate": <f64>}`.
#[derive(Clone, Debug, PartialEq)]
pub enum Magnitude<T> {
    Exact(T),
    Approximate(f64),
}
impl<T: Default> Default for Magnitude<T> {
    fn default() -> Self {
        Self::Exact(T::default())
    }
}
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum MagnitudeWire<T> {
    Exact(T),
    Approximate { approximate: f64 },
}
impl<T: Serialize> Serialize for Magnitude<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Exact(value) => MagnitudeWire::Exact(value),
            Self::Approximate(approximate) => MagnitudeWire::Approximate {
                approximate: *approximate,
            },
        }
        .serialize(serializer)
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Magnitude<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match MagnitudeWire::deserialize(deserializer)? {
            MagnitudeWire::Exact(value) => Self::Exact(value),
            MagnitudeWire::Approximate { approximate } => Self::Approximate(approximate),
        })
    }
}

/// A declared product or quotient.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationDecl {
    pub left: String,
    pub op: ProductOp,
    pub right: String,
    pub result: String,
    #[serde(default)]
    pub commutative: bool,
    #[serde(default)]
    pub provenance: Option<String>,
}
