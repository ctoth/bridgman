use crate::{Dimensions, DynamicQuantity, ExactScalar, ExactValue, QuantityError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};

pub const CATALOG_SCHEMA: u32 = 1;
static NEXT_REGISTRY: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AffineRole {
    Linear,
    Point,
    Difference,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}
impl Op {
    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Div => "div",
        }
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
    pub reference_unit: Option<String>,
    pub scale: Option<ExactScalar>,
    #[serde(default)]
    pub approximate_scale: Option<f64>,
    #[serde(default = "ExactScalar::zero")]
    pub offset: ExactScalar,
    #[serde(default)]
    pub offset_terms: Vec<ExactScalar>,
    #[serde(default)]
    pub approximate_offset: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationDecl {
    pub left: String,
    pub op: Op,
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
pub struct Registry {
    identity: u64,
    kinds: Vec<KindDecl>,
    units: Vec<UnitDecl>,
    kind_ids: HashMap<String, usize>,
    unit_ids: HashMap<String, usize>,
    symbols: HashMap<String, Vec<usize>>,
    operations: HashMap<(usize, Op, usize), (usize, usize)>,
}

impl Registry {
    pub(crate) fn identity(&self) -> u64 {
        self.identity
    }
    pub(crate) fn check_kind_public(&self, h: KindHandle) -> Result<(), QuantityError> {
        self.check_kind(h)
    }
    pub(crate) fn check_unit_public(&self, h: UnitHandle) -> Result<(), QuantityError> {
        if h.registry != self.identity {
            Err(QuantityError::RegistryMismatch)
        } else {
            Ok(())
        }
    }
    pub(crate) fn check_handles(&self, k: KindHandle, u: UnitHandle) -> Result<(), QuantityError> {
        self.check_kind(k)?;
        self.check_unit_public(u)
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
                record: "unit symbol",
                id: symbol.into(),
            })
    }
    pub(crate) fn conversion(
        &self,
        u: UnitHandle,
    ) -> Result<(UnitHandle, f64, f64), QuantityError> {
        self.check_unit_public(u)?;
        let unit = &self.units[u.index];
        let reference = unit.reference_unit.as_ref().ok_or_else(|| {
            QuantityError::InvalidCatalog(format!("unit {:?} has unresolved conversion", unit.id))
        })?;
        let scale = if let Some(value) = &unit.scale {
            value.to_f64().ok_or(QuantityError::NumericalFailure)?
        } else {
            unit.approximate_scale.ok_or_else(|| {
                QuantityError::InvalidCatalog(format!(
                    "unit {:?} has unresolved conversion",
                    unit.id
                ))
            })?
        };
        let offset = if let Some(value) = unit.approximate_offset {
            value
        } else {
            let terms = if unit.offset_terms.is_empty() {
                vec![unit.offset.clone()]
            } else {
                unit.offset_terms.clone()
            };
            ExactValue::from_terms(terms).to_f64()?
        };
        Ok((self.unit(reference)?, scale, offset))
    }
    pub(crate) fn exact_conversion(
        &self,
        u: UnitHandle,
    ) -> Result<(UnitHandle, ExactScalar, ExactValue), QuantityError> {
        self.check_unit_public(u)?;
        let unit = &self.units[u.index];
        let reference = self.unit(unit.reference_unit.as_ref().ok_or_else(|| {
            QuantityError::InvalidCatalog(format!("unit {:?} has unresolved conversion", unit.id))
        })?)?;
        let scale = unit.scale.clone().ok_or_else(|| {
            QuantityError::InvalidCatalog(format!("unit {:?} has approximate conversion", unit.id))
        })?;
        if unit.approximate_offset.is_some() {
            return Err(QuantityError::InvalidCatalog(format!(
                "unit {:?} has approximate conversion",
                unit.id
            )));
        }
        let terms = if unit.offset_terms.is_empty() {
            vec![unit.offset.clone()]
        } else {
            unit.offset_terms.clone()
        };
        Ok((reference, scale, ExactValue::from_terms(terms)))
    }
    pub(crate) fn check_quantity(
        &self,
        q: DynamicQuantity,
        u: UnitHandle,
    ) -> Result<(), QuantityError> {
        if q.registry != self.identity {
            return Err(QuantityError::RegistryMismatch);
        }
        self.check_unit_public(u)?;
        self.require_unit_kind(u, q.kind)
    }
    pub(crate) fn binary(
        &self,
        a: DynamicQuantity,
        op: Op,
        b: DynamicQuantity,
    ) -> Result<DynamicQuantity, QuantityError> {
        if a.registry != self.identity || b.registry != self.identity {
            return Err(QuantityError::RegistryMismatch);
        }
        match op {
            Op::Add | Op::Sub => self.additive(a, op, b),
            Op::Mul | Op::Div => {
                if a.role == AffineRole::Point || b.role == AffineRole::Point {
                    return Err(QuantityError::UnsupportedAffineOperation);
                }
                let result = self.result_kind(a.kind, op, b.kind)?;
                let value = if op == Op::Mul {
                    a.value * b.value
                } else {
                    if b.value == 0.0 {
                        return Err(QuantityError::DivisionByZero);
                    }
                    a.value / b.value
                };
                if !value.is_finite() {
                    return Err(QuantityError::NumericalFailure);
                }
                let unit = self.canonical_unit(result)?;
                Ok(DynamicQuantity {
                    registry: self.identity,
                    kind: result,
                    role: AffineRole::Linear,
                    reference_unit: unit,
                    value,
                })
            }
        }
    }
    fn additive(
        &self,
        a: DynamicQuantity,
        op: Op,
        b: DynamicQuantity,
    ) -> Result<DynamicQuantity, QuantityError> {
        if a.role != AffineRole::Point && b.role != AffineRole::Point && a.kind != b.kind {
            return Err(QuantityError::KindMismatch {
                left: self.kinds[a.kind.index].id.clone(),
                right: self.kinds[b.kind.index].id.clone(),
            });
        }
        if a.reference_unit != b.reference_unit {
            return Err(QuantityError::DisconnectedConversion);
        }
        if a.role == AffineRole::Point && b.role == AffineRole::Point {
            if op == Op::Add {
                return Err(QuantityError::UnsupportedAffineOperation);
            }
            if a.kind != b.kind {
                return Err(QuantityError::KindMismatch {
                    left: self.kinds[a.kind.index].id.clone(),
                    right: self.kinds[b.kind.index].id.clone(),
                });
            }
            let difference = self.kinds[a.kind.index]
                .difference_kind
                .as_ref()
                .ok_or(QuantityError::UnsupportedAffineOperation)?;
            let kind = self.kind(difference)?;
            return Ok(DynamicQuantity {
                registry: self.identity,
                kind,
                role: AffineRole::Difference,
                reference_unit: a.reference_unit,
                value: a.value - b.value,
            });
        }
        if a.role == AffineRole::Point {
            let difference = self.kinds[a.kind.index]
                .difference_kind
                .as_ref()
                .ok_or(QuantityError::UnsupportedAffineOperation)?;
            if self.kind(difference)? != b.kind {
                return Err(QuantityError::KindMismatch {
                    left: difference.clone(),
                    right: self.kinds[b.kind.index].id.clone(),
                });
            }
            let value = if op == Op::Add {
                a.value + b.value
            } else {
                a.value - b.value
            };
            return Ok(DynamicQuantity { value, ..a });
        }
        if b.role == AffineRole::Point {
            return Err(QuantityError::UnsupportedAffineOperation);
        }
        if a.kind != b.kind {
            return Err(QuantityError::KindMismatch {
                left: self.kinds[a.kind.index].id.clone(),
                right: self.kinds[b.kind.index].id.clone(),
            });
        }
        let value = if op == Op::Add {
            a.value + b.value
        } else {
            a.value - b.value
        };
        if !value.is_finite() {
            return Err(QuantityError::NumericalFailure);
        }
        Ok(DynamicQuantity { value, ..a })
    }
    fn canonical_unit(&self, kind: KindHandle) -> Result<UnitHandle, QuantityError> {
        for (index, unit) in self.units.iter().enumerate() {
            if unit.kinds.iter().any(|id| id == &self.kinds[kind.index].id)
                && unit.reference_unit.as_deref() == Some(&unit.id)
                && unit
                    .scale
                    .as_ref()
                    .is_some_and(|v| v == &ExactScalar::one())
                && unit.offset == ExactScalar::zero()
            {
                return Ok(UnitHandle {
                    registry: self.identity,
                    index,
                });
            }
        }
        Err(QuantityError::InvalidCatalog(format!(
            "kind {:?} has no canonical unit",
            self.kinds[kind.index].id
        )))
    }
    pub fn from_json(input: &str) -> Result<Self, QuantityError> {
        let catalog: Catalog = serde_json::from_str(input)
            .map_err(|e| QuantityError::InvalidCatalog(e.to_string()))?;
        Self::compile(catalog)
    }
    pub fn compile(catalog: Catalog) -> Result<Self, QuantityError> {
        if catalog.schema != CATALOG_SCHEMA {
            return Err(QuantityError::Schema {
                expected: CATALOG_SCHEMA,
                actual: catalog.schema,
            });
        }
        let mut kind_ids = HashMap::new();
        for (index, kind) in catalog.kinds.iter().enumerate() {
            if kind.id.is_empty() {
                return Err(QuantityError::InvalidCatalog("kind id is empty".into()));
            }
            if kind_ids.insert(kind.id.clone(), index).is_some() {
                return Err(QuantityError::Duplicate {
                    record: "kind",
                    id: kind.id.clone(),
                });
            }
        }
        for kind in &catalog.kinds {
            if let Some(difference) = &kind.difference_kind {
                let index = *kind_ids
                    .get(difference)
                    .ok_or_else(|| QuantityError::Unknown {
                        record: "kind",
                        id: difference.clone(),
                    })?;
                if kind.dimensions != catalog.kinds[index].dimensions {
                    return Err(QuantityError::InvalidCatalog(format!(
                        "point kind {:?} and difference kind {:?} have different dimensions",
                        kind.id, difference
                    )));
                }
            }
        }
        let mut unit_ids = HashMap::new();
        let mut symbols: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, unit) in catalog.units.iter().enumerate() {
            if unit_ids.insert(unit.id.clone(), index).is_some() {
                return Err(QuantityError::Duplicate {
                    record: "unit",
                    id: unit.id.clone(),
                });
            }
            if unit
                .reference_unit
                .as_ref()
                .is_some_and(|reference| !unit_ids.contains_key(reference) && reference != &unit.id)
            {
                // Forward references are checked after the complete index exists.
            }
            for kind in &unit.kinds {
                if !kind_ids.contains_key(kind) {
                    return Err(QuantityError::Unknown {
                        record: "kind",
                        id: kind.clone(),
                    });
                }
            }
            symbols.entry(unit.symbol.clone()).or_default().push(index);
        }
        for unit in &catalog.units {
            if let Some(reference) = &unit.reference_unit {
                if !unit_ids.contains_key(reference) {
                    return Err(QuantityError::Unknown {
                        record: "unit",
                        id: reference.clone(),
                    });
                }
            }
        }
        let mut operations = HashMap::new();
        for (declaration_index, operation) in catalog.operations.iter().enumerate() {
            let left = *kind_ids
                .get(&operation.left)
                .ok_or_else(|| QuantityError::Unknown {
                    record: "kind",
                    id: operation.left.clone(),
                })?;
            let right = *kind_ids
                .get(&operation.right)
                .ok_or_else(|| QuantityError::Unknown {
                    record: "kind",
                    id: operation.right.clone(),
                })?;
            let result =
                *kind_ids
                    .get(&operation.result)
                    .ok_or_else(|| QuantityError::Unknown {
                        record: "kind",
                        id: operation.result.clone(),
                    })?;
            let left_dims = catalog.kinds[left]
                .dimensions
                .as_ref()
                .ok_or_else(|| QuantityError::UnresolvedDimensions(operation.left.clone()))?;
            let right_dims = catalog.kinds[right]
                .dimensions
                .as_ref()
                .ok_or_else(|| QuantityError::UnresolvedDimensions(operation.right.clone()))?;
            let result_dims = catalog.kinds[result]
                .dimensions
                .as_ref()
                .ok_or_else(|| QuantityError::UnresolvedDimensions(operation.result.clone()))?;
            let expected = match operation.op {
                Op::Mul => left_dims * right_dims,
                Op::Div => left_dims / right_dims,
                Op::Add | Op::Sub => left_dims.clone(),
            };
            if expected != *result_dims
                || matches!(operation.op, Op::Add | Op::Sub) && left_dims != right_dims
            {
                return Err(QuantityError::InvalidOperationRule);
            }
            insert_operation(
                &mut operations,
                (left, operation.op, right),
                (result, declaration_index),
                &catalog,
            )?;
            if operation.commutative && left != right {
                insert_operation(
                    &mut operations,
                    (right, operation.op, left),
                    (result, declaration_index),
                    &catalog,
                )?;
            }
        }
        Ok(Self {
            identity: NEXT_REGISTRY.fetch_add(1, Ordering::Relaxed),
            kinds: catalog.kinds,
            units: catalog.units,
            kind_ids,
            unit_ids,
            symbols,
            operations,
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
                record: "kind",
                id: id.into(),
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
                record: "unit",
                id: id.into(),
            })
    }
    pub fn kinds_for_symbol(&self, symbol: &str) -> Result<Vec<KindHandle>, QuantityError> {
        let units = self
            .symbols
            .get(symbol)
            .ok_or_else(|| QuantityError::Unknown {
                record: "unit symbol",
                id: symbol.into(),
            })?;
        let mut result = Vec::new();
        for &unit in units {
            for kind in &self.units[unit].kinds {
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
        op: Op,
        right: KindHandle,
    ) -> Result<KindHandle, QuantityError> {
        self.check_kind(left)?;
        self.check_kind(right)?;
        let &(index, _) = self
            .operations
            .get(&(left.index, op, right.index))
            .ok_or_else(|| QuantityError::MissingOperationRule {
                left: self.kinds[left.index].id.clone(),
                op: op.name(),
                right: self.kinds[right.index].id.clone(),
            })?;
        Ok(KindHandle {
            registry: self.identity,
            index,
        })
    }
    fn check_kind(&self, h: KindHandle) -> Result<(), QuantityError> {
        if h.registry != self.identity {
            Err(QuantityError::RegistryMismatch)
        } else {
            Ok(())
        }
    }
}

fn insert_operation(
    map: &mut HashMap<(usize, Op, usize), (usize, usize)>,
    key: (usize, Op, usize),
    value: (usize, usize),
    catalog: &Catalog,
) -> Result<(), QuantityError> {
    if map.insert(key, value).is_some() {
        return Err(QuantityError::ConflictingOperationRule {
            left: catalog.kinds[key.0].id.clone(),
            op: key.1.name(),
            right: catalog.kinds[key.2].id.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn catalog() -> Catalog {
        Catalog {
            schema: 1,
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
                op: Op::Mul,
                right: "length".into(),
                result: "area".into(),
                commutative: false,
                provenance: None,
            }],
        }
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
            r.kind_id(r.result_kind(length, Op::Mul, length).unwrap())
                .unwrap(),
            "area"
        );
    }
}
