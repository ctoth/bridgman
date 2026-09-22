use crate::{Dimensions, DynamicQuantity, ExactScalar, ExactValue, QuantityError, Record};
use num_traits::Zero;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

pub const CATALOG_SCHEMA: u32 = 2;
static NEXT_REGISTRY: AtomicU64 = AtomicU64::new(1);

/// How a kind takes part in additive arithmetic. It follows from the declared
/// affine spaces: a kind naming a `difference_kind` is a point kind, a kind
/// named as one is a difference kind, and every other kind is linear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AffineRole {
    Linear,
    Point,
    Difference,
}

/// A binary operation of quantity arithmetic. `name` is its only spelling:
/// serialization, parsing and display all read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}
impl Op {
    const ALL: [Self; 4] = [Self::Add, Self::Sub, Self::Mul, Self::Div];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Div => "div",
        }
    }
    /// The product or quotient this operation is, if it is one.
    pub const fn product(self) -> Option<ProductOp> {
        match self {
            Self::Mul => Some(ProductOp::Mul),
            Self::Div => Some(ProductOp::Div),
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
}
impl From<ProductOp> for Op {
    fn from(op: ProductOp) -> Self {
        match op {
            ProductOp::Mul => Self::Mul,
            ProductOp::Div => Self::Div,
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
    #[serde(default)]
    pub difference_kind: Option<String>,
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
    fn is_terminal(&self) -> bool {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KindHandle {
    registry: u64,
    index: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnitHandle {
    registry: u64,
    index: usize,
}

#[derive(Clone, Debug)]
struct CompiledKind {
    id: String,
    dimensions: Option<Dimensions>,
    role: AffineRole,
    /// A point kind's declared difference kind.
    difference: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Registry {
    identity: u64,
    kinds: Vec<CompiledKind>,
    units: Vec<UnitDecl>,
    kind_ids: HashMap<String, usize>,
    unit_ids: HashMap<String, usize>,
    symbols: HashMap<String, Vec<usize>>,
    operations: HashMap<(usize, ProductOp, usize), usize>,
    provenance: BTreeMap<String, String>,
}

impl Registry {
    pub fn provenance(&self) -> &BTreeMap<String, String> {
        &self.provenance
    }

    fn coherent_scale(&self, unit: UnitHandle) -> Result<ExactScalar, QuantityError> {
        let unit = &self.units[unit.index];
        unit.coherent_scale
            .clone()
            .ok_or_else(|| QuantityError::MissingCoherentScale {
                unit: unit.id.clone(),
            })
    }
    pub(crate) fn identity(&self) -> u64 {
        self.identity
    }
    pub(crate) fn owns(&self, registry: u64) -> Result<(), QuantityError> {
        if registry == self.identity {
            Ok(())
        } else {
            Err(QuantityError::RegistryMismatch)
        }
    }
    pub(crate) fn check_kind(&self, h: KindHandle) -> Result<(), QuantityError> {
        self.owns(h.registry)
    }
    pub(crate) fn check_unit(&self, h: UnitHandle) -> Result<(), QuantityError> {
        self.owns(h.registry)
    }
    pub(crate) fn unit_has_kind(&self, u: UnitHandle, k: KindHandle) -> bool {
        self.units[u.index]
            .kinds
            .iter()
            .any(|id| id == &self.kinds[k.index].id)
    }
    pub(crate) fn require_unit_kind(
        &self,
        u: UnitHandle,
        k: KindHandle,
    ) -> Result<(), QuantityError> {
        if self.unit_has_kind(u, k) {
            Ok(())
        } else {
            Err(QuantityError::UnitKindMismatch {
                unit: self.units[u.index].id.clone(),
                kind: self.kinds[k.index].id.clone(),
            })
        }
    }
    pub(crate) fn units_for_symbol(&self, symbol: &str) -> Result<Vec<UnitHandle>, QuantityError> {
        self.symbols
            .get(symbol)
            .map(|v| {
                v.iter()
                    .map(|&index| UnitHandle {
                        registry: self.identity,
                        index,
                    })
                    .collect()
            })
            .ok_or_else(|| QuantityError::Unknown {
                record: Record::UnitSymbol,
                id: symbol.into(),
            })
    }
    fn declared_conversion(&self, u: UnitHandle) -> Result<&Conversion, QuantityError> {
        self.check_unit(u)?;
        let unit = &self.units[u.index];
        unit.conversion
            .as_ref()
            .ok_or_else(|| QuantityError::UnresolvedConversion {
                unit: unit.id.clone(),
            })
    }
    pub(crate) fn conversion(
        &self,
        u: UnitHandle,
    ) -> Result<(UnitHandle, f64, f64), QuantityError> {
        let conversion = self.declared_conversion(u)?;
        let scale = match &conversion.scale {
            Magnitude::Exact(value) => value.to_f64().ok_or(QuantityError::NumericalFailure)?,
            Magnitude::Approximate(value) => *value,
        };
        let offset = match &conversion.offset {
            Magnitude::Exact(value) => value.to_f64()?,
            Magnitude::Approximate(value) => *value,
        };
        if !scale.is_finite() || scale == 0.0 || !offset.is_finite() {
            return Err(QuantityError::NumericalFailure);
        }
        Ok((self.unit(&conversion.reference_unit)?, scale, offset))
    }
    pub(crate) fn exact_conversion(
        &self,
        u: UnitHandle,
    ) -> Result<(UnitHandle, ExactScalar, ExactValue), QuantityError> {
        let conversion = self.declared_conversion(u)?;
        let (Magnitude::Exact(scale), Magnitude::Exact(offset)) =
            (&conversion.scale, &conversion.offset)
        else {
            return Err(QuantityError::ApproximateConversion {
                unit: self.units[u.index].id.clone(),
            });
        };
        Ok((
            self.unit(&conversion.reference_unit)?,
            scale.clone(),
            offset.clone(),
        ))
    }
    pub(crate) fn check_quantity(
        &self,
        q: DynamicQuantity,
        u: UnitHandle,
    ) -> Result<(), QuantityError> {
        self.owns(q.registry)?;
        self.check_unit(u)?;
        self.require_unit_kind(u, q.kind)
    }
    pub(crate) fn binary(
        &self,
        a: DynamicQuantity,
        op: Op,
        b: DynamicQuantity,
    ) -> Result<DynamicQuantity, QuantityError> {
        self.owns(a.registry)?;
        self.owns(b.registry)?;
        match op.product() {
            Some(product) => self.product(a, product, b),
            None => self.additive(a, op == Op::Add, b),
        }
    }
    fn product(
        &self,
        a: DynamicQuantity,
        op: ProductOp,
        b: DynamicQuantity,
    ) -> Result<DynamicQuantity, QuantityError> {
        if self.kinds[a.kind.index].role == AffineRole::Point
            || self.kinds[b.kind.index].role == AffineRole::Point
        {
            return Err(QuantityError::UnsupportedAffineOperation);
        }
        let result = self.result_kind(a.kind, op, b.kind)?;
        if self.kinds[result.index].role == AffineRole::Point {
            return Err(QuantityError::UnsupportedAffineOperation);
        }
        let unit = self.canonical_unit(result)?;
        let a_scale = self.coherent_scale(a.reference_unit)?;
        let b_scale = self.coherent_scale(b.reference_unit)?;
        let result_scale = self.coherent_scale(unit)?;
        let (factor, value) = match op {
            ProductOp::Mul => (a_scale.multiply(&b_scale), a.value * b.value),
            ProductOp::Div => {
                if b.value == 0.0 {
                    return Err(QuantityError::DivisionByZero);
                }
                (
                    a_scale
                        .divide(&b_scale)
                        .ok_or(QuantityError::DivisionByZero)?,
                    a.value / b.value,
                )
            }
        };
        let factor = factor
            .divide(&result_scale)
            .ok_or(QuantityError::DivisionByZero)?
            .to_f64()
            .ok_or(QuantityError::NumericalFailure)?;
        let value = value * factor;
        if !value.is_finite() {
            return Err(QuantityError::NumericalFailure);
        }
        Ok(DynamicQuantity {
            registry: self.identity,
            kind: result,
            reference_unit: unit,
            value,
        })
    }
    fn additive(
        &self,
        a: DynamicQuantity,
        add: bool,
        b: DynamicQuantity,
    ) -> Result<DynamicQuantity, QuantityError> {
        let (left, right) = (&self.kinds[a.kind.index], &self.kinds[b.kind.index]);
        let mismatch = |left: &str| QuantityError::KindMismatch {
            left: left.into(),
            right: right.id.clone(),
        };
        let kind = match (left.role, right.role) {
            (AffineRole::Point, AffineRole::Point) => {
                if add {
                    return Err(QuantityError::UnsupportedAffineOperation);
                }
                if a.kind != b.kind {
                    return Err(mismatch(&left.id));
                }
                left.difference
                    .ok_or(QuantityError::UnsupportedAffineOperation)?
            }
            (AffineRole::Point, _) => {
                let difference = left
                    .difference
                    .ok_or(QuantityError::UnsupportedAffineOperation)?;
                if difference != b.kind.index {
                    return Err(mismatch(&self.kinds[difference].id));
                }
                a.kind.index
            }
            (_, AffineRole::Point) => {
                return if add {
                    self.additive(b, add, a)
                } else {
                    Err(QuantityError::UnsupportedAffineOperation)
                };
            }
            _ => {
                if a.kind != b.kind {
                    return Err(mismatch(&left.id));
                }
                a.kind.index
            }
        };
        if a.reference_unit != b.reference_unit {
            return Err(QuantityError::DisconnectedConversion);
        }
        let value = if add {
            a.value + b.value
        } else {
            a.value - b.value
        };
        if !value.is_finite() {
            return Err(QuantityError::NumericalFailure);
        }
        Ok(DynamicQuantity {
            registry: self.identity,
            kind: KindHandle {
                registry: self.identity,
                index: kind,
            },
            reference_unit: a.reference_unit,
            value,
        })
    }
    fn canonical_unit(&self, kind: KindHandle) -> Result<UnitHandle, QuantityError> {
        let id = &self.kinds[kind.index].id;
        self.units
            .iter()
            .position(|unit| unit.kinds.contains(id) && unit.is_terminal())
            .map(|index| UnitHandle {
                registry: self.identity,
                index,
            })
            .ok_or_else(|| QuantityError::NoCanonicalUnit { kind: id.clone() })
    }
    pub fn from_json(input: &str) -> Result<Self, QuantityError> {
        let catalog: Catalog = serde_json::from_str(input)
            .map_err(|error| QuantityError::CatalogJson(error.into()))?;
        Self::compile(catalog)
    }
    pub fn compile(catalog: Catalog) -> Result<Self, QuantityError> {
        if catalog.schema != CATALOG_SCHEMA {
            return Err(QuantityError::Schema {
                expected: CATALOG_SCHEMA,
                actual: catalog.schema,
            });
        }
        let kinds = compile_kinds(catalog.kinds)?;
        let kind_ids: HashMap<String, usize> = kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| (kind.id.clone(), index))
            .collect();
        let mut unit_ids = HashMap::new();
        let mut symbols: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, unit) in catalog.units.iter().enumerate() {
            if unit.id.is_empty() {
                return Err(QuantityError::EmptyId {
                    record: Record::Unit,
                });
            }
            check_magnitudes(unit)?;
            if unit_ids.insert(unit.id.clone(), index).is_some() {
                return Err(QuantityError::Duplicate {
                    record: Record::Unit,
                    id: unit.id.clone(),
                });
            }
            for kind in &unit.kinds {
                if !kind_ids.contains_key(kind) {
                    return Err(QuantityError::Unknown {
                        record: Record::Kind,
                        id: kind.clone(),
                    });
                }
            }
            symbols.entry(unit.symbol.clone()).or_default().push(index);
        }
        for unit in &catalog.units {
            let Some(conversion) = &unit.conversion else {
                continue;
            };
            let reference = &conversion.reference_unit;
            let terminal =
                &catalog.units[*unit_ids
                    .get(reference)
                    .ok_or_else(|| QuantityError::Unknown {
                        record: Record::Unit,
                        id: reference.clone(),
                    })?];
            for kind in &unit.kinds {
                if let Some(dimensions) = &kinds[kind_ids[kind]].dimensions {
                    if !terminal.kinds.iter().any(|target| {
                        kinds[kind_ids[target]].dimensions.as_ref() == Some(dimensions)
                    }) {
                        return Err(QuantityError::IncompatibleReference {
                            unit: unit.id.clone(),
                            reference: reference.clone(),
                            kind: kind.clone(),
                        });
                    }
                }
            }
            if !terminal.is_terminal() {
                return Err(QuantityError::NonIdentityReference {
                    unit: unit.id.clone(),
                    reference: reference.clone(),
                });
            }
        }
        let mut operations = HashMap::new();
        for operation in &catalog.operations {
            if operation.commutative && operation.op == ProductOp::Div {
                return Err(QuantityError::InvalidOperationRule);
            }
            let index = |id: &String| {
                kind_ids
                    .get(id)
                    .copied()
                    .ok_or_else(|| QuantityError::Unknown {
                        record: Record::Kind,
                        id: id.clone(),
                    })
            };
            let dimensions = |index: usize| {
                kinds[index]
                    .dimensions
                    .as_ref()
                    .ok_or_else(|| QuantityError::UnresolvedDimensions(kinds[index].id.clone()))
            };
            let left = index(&operation.left)?;
            let right = index(&operation.right)?;
            let result = index(&operation.result)?;
            let expected = match operation.op {
                ProductOp::Mul => dimensions(left)? * dimensions(right)?,
                ProductOp::Div => dimensions(left)? / dimensions(right)?,
            };
            if expected != *dimensions(result)? {
                return Err(QuantityError::InvalidOperationRule);
            }
            let mut insert = |left: usize, right: usize| {
                if operations
                    .insert((left, operation.op, right), result)
                    .is_some()
                {
                    Err(QuantityError::ConflictingOperationRule {
                        left: kinds[left].id.clone(),
                        op: operation.op,
                        right: kinds[right].id.clone(),
                    })
                } else {
                    Ok(())
                }
            };
            insert(left, right)?;
            if operation.commutative && left != right {
                insert(right, left)?;
            }
        }
        Ok(Self {
            identity: NEXT_REGISTRY.fetch_add(1, Ordering::Relaxed),
            kinds,
            units: catalog.units,
            kind_ids,
            unit_ids,
            symbols,
            operations,
            provenance: catalog.provenance,
        })
    }
    pub fn kind(&self, id: &str) -> Result<KindHandle, QuantityError> {
        self.kind_ids
            .get(id)
            .map(|&index| KindHandle {
                registry: self.identity,
                index,
            })
            .ok_or_else(|| QuantityError::Unknown {
                record: Record::Kind,
                id: id.into(),
            })
    }
    /// Every declared kind, in declaration order.
    pub fn kinds(&self) -> impl Iterator<Item = KindHandle> + '_ {
        (0..self.kinds.len()).map(|index| KindHandle {
            registry: self.identity,
            index,
        })
    }
    pub fn kind_count(&self) -> usize {
        self.kinds.len()
    }
    pub fn unit_count(&self) -> usize {
        self.units.len()
    }
    pub fn unit(&self, id: &str) -> Result<UnitHandle, QuantityError> {
        self.unit_ids
            .get(id)
            .map(|&index| UnitHandle {
                registry: self.identity,
                index,
            })
            .ok_or_else(|| QuantityError::Unknown {
                record: Record::Unit,
                id: id.into(),
            })
    }
    pub fn kinds_for_symbol(&self, symbol: &str) -> Result<Vec<KindHandle>, QuantityError> {
        let mut result = Vec::new();
        for unit in self.units_for_symbol(symbol)? {
            for kind in &self.units[unit.index].kinds {
                let handle = self.kind(kind)?;
                if !result.contains(&handle) {
                    result.push(handle);
                }
            }
        }
        Ok(result)
    }
    pub fn kind_id(&self, handle: KindHandle) -> Result<&str, QuantityError> {
        self.check_kind(handle)?;
        Ok(&self.kinds[handle.index].id)
    }
    pub fn role(&self, handle: KindHandle) -> Result<AffineRole, QuantityError> {
        self.check_kind(handle)?;
        Ok(self.kinds[handle.index].role)
    }
    pub fn dimensions(&self, handle: KindHandle) -> Result<&Dimensions, QuantityError> {
        self.check_kind(handle)?;
        self.kinds[handle.index]
            .dimensions
            .as_ref()
            .ok_or_else(|| QuantityError::UnresolvedDimensions(self.kinds[handle.index].id.clone()))
    }
    pub fn result_kind(
        &self,
        left: KindHandle,
        op: ProductOp,
        right: KindHandle,
    ) -> Result<KindHandle, QuantityError> {
        self.check_kind(left)?;
        self.check_kind(right)?;
        let &index = self
            .operations
            .get(&(left.index, op, right.index))
            .ok_or_else(|| QuantityError::MissingOperationRule {
                left: self.kinds[left.index].id.clone(),
                op,
                right: self.kinds[right.index].id.clone(),
            })?;
        Ok(KindHandle {
            registry: self.identity,
            index,
        })
    }
}

/// Resolve kind identities and affine spaces, fixing each kind's role once.
fn compile_kinds(declarations: Vec<KindDecl>) -> Result<Vec<CompiledKind>, QuantityError> {
    let mut ids = HashMap::new();
    for (index, kind) in declarations.iter().enumerate() {
        if kind.id.is_empty() {
            return Err(QuantityError::EmptyId {
                record: Record::Kind,
            });
        }
        if ids.insert(kind.id.as_str(), index).is_some() {
            return Err(QuantityError::Duplicate {
                record: Record::Kind,
                id: kind.id.clone(),
            });
        }
    }
    let mut differences = Vec::with_capacity(declarations.len());
    for kind in &declarations {
        let difference = match &kind.difference_kind {
            None => None,
            Some(id) => {
                let index = *ids.get(id.as_str()).ok_or_else(|| QuantityError::Unknown {
                    record: Record::Kind,
                    id: id.clone(),
                })?;
                let target = &declarations[index];
                if kind.dimensions != target.dimensions {
                    return Err(QuantityError::AffineDimensionMismatch {
                        point: kind.id.clone(),
                        difference: id.clone(),
                    });
                }
                if target.difference_kind.is_some() {
                    return Err(QuantityError::NestedAffineSpace {
                        point: kind.id.clone(),
                        difference: id.clone(),
                    });
                }
                Some(index)
            }
        };
        differences.push(difference);
    }
    let difference_kinds: HashSet<usize> = differences.iter().flatten().copied().collect();
    Ok(declarations
        .into_iter()
        .zip(differences)
        .enumerate()
        .map(|(index, (kind, difference))| CompiledKind {
            role: if difference.is_some() {
                AffineRole::Point
            } else if difference_kinds.contains(&index) {
                AffineRole::Difference
            } else {
                AffineRole::Linear
            },
            id: kind.id,
            dimensions: kind.dimensions,
            difference,
        })
        .collect())
}

fn check_magnitudes(unit: &UnitDecl) -> Result<(), QuantityError> {
    let zero_scale = || QuantityError::ZeroScale {
        unit: unit.id.clone(),
    };
    let nonfinite = || QuantityError::NonFiniteConversion {
        unit: unit.id.clone(),
    };
    if unit
        .coherent_scale
        .as_ref()
        .is_some_and(|s| s.rational.is_zero())
    {
        return Err(zero_scale());
    }
    let Some(conversion) = &unit.conversion else {
        return Ok(());
    };
    match &conversion.scale {
        Magnitude::Exact(scale) if scale.rational.is_zero() => return Err(zero_scale()),
        Magnitude::Approximate(scale) if !scale.is_finite() => return Err(nonfinite()),
        Magnitude::Approximate(scale) if *scale == 0.0 => return Err(zero_scale()),
        Magnitude::Exact(_) | Magnitude::Approximate(_) => {}
    }
    match &conversion.offset {
        Magnitude::Approximate(offset) if !offset.is_finite() => Err(nonfinite()),
        Magnitude::Exact(_) | Magnitude::Approximate(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> Catalog {
        Catalog {
            schema: CATALOG_SCHEMA,
            provenance: BTreeMap::new(),
            kinds: vec![
                KindDecl {
                    id: "length".into(),
                    dimensions: Some(Dimensions::from_integer_powers([("L", 1)])),
                    difference_kind: None,
                },
                KindDecl {
                    id: "area".into(),
                    dimensions: Some(Dimensions::from_integer_powers([("L", 2)])),
                    difference_kind: None,
                },
            ],
            units: vec![],
            operations: vec![OperationDecl {
                left: "length".into(),
                op: ProductOp::Mul,
                right: "length".into(),
                result: "area".into(),
                commutative: false,
                provenance: None,
            }],
        }
    }
    #[test]
    fn products_use_coherent_scales_instead_of_reference_magnitudes() {
        let mut c = catalog();
        let unit = |id: &str, kind: &str, coherent: &str| UnitDecl {
            id: id.into(),
            symbol: id.into(),
            kinds: vec![kind.into()],
            conversion: Some(Conversion {
                reference_unit: id.into(),
                scale: Magnitude::Exact(ExactScalar::one()),
                offset: Magnitude::default(),
            }),
            coherent_scale: Some(ExactScalar::parse(coherent).unwrap()),
        };
        c.units = vec![unit("cm", "length", "1/100"), unit("m2", "area", "1")];
        let r = Registry::compile(c).unwrap();
        let length = r
            .quantity(100.0, r.unit("cm").unwrap(), r.kind("length").unwrap())
            .unwrap();
        assert_eq!(
            length
                .mul(&r, length)
                .unwrap()
                .in_unit(&r, r.unit("m2").unwrap())
                .unwrap(),
            1.0
        );
    }
    #[test]
    fn invalid_conversion_and_commutative_division_are_rejected() {
        let mut c = catalog();
        c.operations[0].op = ProductOp::Div;
        c.operations[0].commutative = true;
        assert!(matches!(
            Registry::compile(c),
            Err(QuantityError::InvalidOperationRule)
        ));
        let c = r#"{"schema":2,"kinds":[{"id":"x","dimensions":{}}],"units":[{"id":"u","symbol":"u","kinds":["x"],"conversion":{"reference_unit":"u","scale":"0"}}]}"#;
        assert_eq!(
            Registry::from_json(c).unwrap_err(),
            QuantityError::ZeroScale { unit: "u".into() }
        );
    }
    #[test]
    fn same_kind_symbol_collision_requires_a_unit_handle() {
        let text = r#"{
          "schema":2,"kinds":[{"id":"length","dimensions":{"L":"1"}}],
          "units":[
            {"id":"u1","symbol":"u","kinds":["length"],"conversion":{"reference_unit":"u1","scale":"1"}},
            {"id":"u2","symbol":"u","kinds":["length"],"conversion":{"reference_unit":"u1","scale":"2"}}
          ]
        }"#;
        let r = Registry::from_json(text).unwrap();
        assert_eq!(
            r.quantity_for_symbol(1.0, "u", Some(r.kind("length").unwrap())),
            Err(QuantityError::AmbiguousUnit("u".into()))
        );
        assert_eq!(
            r.quantity(1.0, r.unit("u2").unwrap(), r.kind("length").unwrap())
                .unwrap()
                .in_unit(&r, r.unit("u1").unwrap())
                .unwrap(),
            2.0
        );
    }
    #[test]
    fn linear_kind_cannot_silently_discard_offset() {
        let r = Registry::from_json(r#"{
          "schema":2,"kinds":[{"id":"coordinate","dimensions":{"X":1}}],
          "units":[
            {"id":"base","symbol":"base","kinds":["coordinate"],"conversion":{"reference_unit":"base","scale":"1"}},
            {"id":"shifted","symbol":"shifted","kinds":["coordinate"],"conversion":{"reference_unit":"base","scale":"1","offset":["10"]}}
          ]
        }"#).unwrap();
        assert_eq!(
            r.quantity(
                2.0,
                r.unit("shifted").unwrap(),
                r.kind("coordinate").unwrap()
            ),
            Err(QuantityError::UnsupportedAffineOperation)
        );
    }
    #[test]
    fn declarations_cannot_name_additive_rules() {
        let text = r#"{"schema":2,"kinds":[{"id":"x","dimensions":{}}],"units":[],
          "operations":[{"left":"x","op":"add","right":"x","result":"x"}]}"#;
        assert!(matches!(
            Registry::from_json(text),
            Err(QuantityError::CatalogJson(_))
        ));
        assert_eq!(
            "add".parse::<ProductOp>(),
            Err(OperationParseError::NotProduct(Op::Add))
        );
    }
    #[test]
    fn operation_names_are_spelled_once() {
        for op in Op::ALL {
            assert_eq!(op.name().parse::<Op>(), Ok(op));
            assert_eq!(
                serde_json::to_string(&op).unwrap(),
                format!("\"{}\"", op.name())
            );
        }
    }
    #[test]
    fn approximate_magnitudes_round_trip_and_refuse_exact_conversion() {
        let text = r#"{"schema":2,"kinds":[{"id":"x","dimensions":{}}],"units":[
            {"id":"u","symbol":"u","kinds":["x"],"conversion":{"reference_unit":"u","scale":"1"}},
            {"id":"v","symbol":"v","kinds":["x"],"conversion":{"reference_unit":"u","scale":{"approximate":2.5}}}
          ]}"#;
        let r = Registry::from_json(text).unwrap();
        let x = r.kind("x").unwrap();
        let (u, v) = (r.unit("u").unwrap(), r.unit("v").unwrap());
        assert_eq!(r.quantity(2.0, v, x).unwrap().in_unit(&r, u).unwrap(), 5.0);
        assert_eq!(
            r.convert_exact(ExactValue::default(), v, u, x),
            Err(QuantityError::ApproximateConversion { unit: "v".into() })
        );
    }
    #[test]
    fn conversion_reference_dimensions_must_match() {
        let r = Registry::from_json(
            r#"{
          "schema":2,"kinds":[{"id":"length","dimensions":{"L":1}},{"id":"time","dimensions":{"T":1}}],
          "units":[
            {"id":"second","symbol":"s","kinds":["time"],"conversion":{"reference_unit":"second","scale":"1"}},
            {"id":"metre","symbol":"m","kinds":["length"],"conversion":{"reference_unit":"second","scale":"1"}}
          ]
        }"#,
        );
        assert!(matches!(
            r,
            Err(QuantityError::IncompatibleReference { .. })
        ));
    }
    #[test]
    fn nested_affine_spaces_are_rejected() {
        let mut c = catalog();
        c.kinds[0].difference_kind = Some("length".into());
        assert_eq!(
            Registry::compile(c).unwrap_err(),
            QuantityError::NestedAffineSpace {
                point: "length".into(),
                difference: "length".into()
            }
        );
    }
    #[test]
    fn foreign_handles_fail() {
        let a = Registry::compile(catalog()).unwrap();
        let b = Registry::compile(catalog()).unwrap();
        assert_eq!(
            b.dimensions(a.kind("length").unwrap()),
            Err(QuantityError::RegistryMismatch)
        );
    }
    #[test]
    fn exact_rule_is_selected() {
        let r = Registry::compile(catalog()).unwrap();
        let length = r.kind("length").unwrap();
        assert_eq!(
            r.kind_id(r.result_kind(length, ProductOp::Mul, length).unwrap())
                .unwrap(),
            "area"
        );
    }
}
