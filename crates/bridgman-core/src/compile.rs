//! Checking a catalog's declarations and compiling them into a `Registry`.
//! Every refusal here is a `CatalogError`: a fault of the declarations, found
//! before any quantity exists.
use crate::catalog::{Catalog, KindDecl, Magnitude, ProductOp, UnitDecl, CATALOG_SCHEMA};
use crate::registry::CompiledKind;
use crate::{AffineRole, CatalogError, Dimensions, Record, Registry};
use num_traits::Zero;
use std::collections::{HashMap, HashSet};

impl Registry {
    pub fn from_json(input: &str) -> Result<Self, CatalogError> {
        let catalog: Catalog =
            serde_json::from_str(input).map_err(|error| CatalogError::Json(error.into()))?;
        Self::compile(catalog)
    }
    pub fn from_yaml(input: &str) -> Result<Self, CatalogError> {
        // Through `Value`, which rejects duplicate mapping keys.
        let yaml = |error: serde_yaml::Error| CatalogError::Yaml(error.into());
        let value: serde_yaml::Value = serde_yaml::from_str(input).map_err(yaml)?;
        Self::compile(serde_yaml::from_value(value).map_err(yaml)?)
    }
    pub fn compile(catalog: Catalog) -> Result<Self, CatalogError> {
        if catalog.schema != CATALOG_SCHEMA {
            return Err(CatalogError::Schema {
                expected: CATALOG_SCHEMA,
                actual: catalog.schema,
            });
        }
        let mut kinds = compile_kinds(&catalog.kinds)?;
        let kind_ids: HashMap<String, usize> = kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| (kind.id.clone(), index))
            .collect();
        let known_kind = |id: &String| {
            kind_ids
                .get(id)
                .copied()
                .ok_or_else(|| CatalogError::Unknown {
                    record: Record::Kind,
                    id: id.clone(),
                })
        };
        let mut unit_ids = HashMap::new();
        let mut symbols: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, unit) in catalog.units.iter().enumerate() {
            if unit.id.is_empty() {
                return Err(CatalogError::EmptyId {
                    record: Record::Unit,
                });
            }
            check_magnitudes(unit)?;
            if unit_ids.insert(unit.id.clone(), index).is_some() {
                return Err(CatalogError::Duplicate {
                    record: Record::Unit,
                    id: unit.id.clone(),
                });
            }
            for kind in &unit.kinds {
                let kind = known_kind(kind)?;
                if unit.is_terminal() && kinds[kind].canonical.is_none() {
                    kinds[kind].canonical = Some(index);
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
                    .ok_or_else(|| CatalogError::Unknown {
                        record: Record::Unit,
                        id: reference.clone(),
                    })?];
            for kind in &unit.kinds {
                if let Some(dimensions) = &kinds[kind_ids[kind]].dimensions {
                    if !terminal.kinds.iter().any(|target| {
                        kinds[kind_ids[target]].dimensions.as_ref() == Some(dimensions)
                    }) {
                        return Err(CatalogError::IncompatibleReference {
                            unit: unit.id.clone(),
                            reference: reference.clone(),
                            kind: kind.clone(),
                        });
                    }
                }
            }
            if !terminal.is_terminal() {
                return Err(CatalogError::NonIdentityReference {
                    unit: unit.id.clone(),
                    reference: reference.clone(),
                });
            }
        }
        // A least value is stated in the canonical unit, so every unit of the
        // kind must reach that unit.
        for (declaration, kind) in catalog.kinds.iter().zip(&mut kinds) {
            let Some(minimum) = &declaration.minimum else {
                continue;
            };
            let invalid = || CatalogError::InvalidMinimum {
                kind: kind.id.clone(),
            };
            let canonical = kind.canonical.ok_or_else(invalid)?;
            let off_canonical = catalog.units.iter().find(|unit| {
                unit.kinds.contains(&kind.id)
                    && unit
                        .conversion
                        .as_ref()
                        .is_none_or(|c| c.reference_unit != catalog.units[canonical].id)
            });
            if off_canonical.is_some() {
                return Err(invalid());
            }
            kind.minimum = Some(minimum.to_f64().ok_or_else(invalid)?);
        }
        let dimensionless = match &catalog.dimensionless {
            None => None,
            Some(id) => {
                let index = known_kind(id)?;
                let kind = &kinds[index];
                if kind.role != AffineRole::Linear
                    || kind.dimensions.as_ref() != Some(&Dimensions::one())
                {
                    return Err(CatalogError::InvalidDimensionlessKind(id.clone()));
                }
                Some(index)
            }
        };
        let mut operations = HashMap::new();
        for operation in &catalog.operations {
            if operation.commutative && operation.op == ProductOp::Div {
                return Err(CatalogError::InvalidOperationRule);
            }
            let dimensions = |index: usize| {
                kinds[index]
                    .dimensions
                    .as_ref()
                    .ok_or_else(|| CatalogError::UnresolvedDimensions(kinds[index].id.clone()))
            };
            let left = known_kind(&operation.left)?;
            let right = known_kind(&operation.right)?;
            let result = known_kind(&operation.result)?;
            let expected = match operation.op {
                ProductOp::Mul => dimensions(left)? * dimensions(right)?,
                ProductOp::Div => dimensions(left)? / dimensions(right)?,
            };
            if expected != *dimensions(result)? {
                return Err(CatalogError::InvalidOperationRule);
            }
            let mut insert = |left: usize, right: usize| {
                if operations
                    .insert((left, operation.op, right), result)
                    .is_some()
                {
                    Err(CatalogError::ConflictingOperationRule {
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
            kinds,
            units: catalog.units,
            kind_ids,
            unit_ids,
            symbols,
            operations,
            dimensionless,
            provenance: catalog.provenance,
        })
    }
}

/// Resolve kind identities and affine spaces, fixing each kind's role once.
fn compile_kinds(declarations: &[KindDecl]) -> Result<Vec<CompiledKind>, CatalogError> {
    let mut ids = HashMap::new();
    for (index, kind) in declarations.iter().enumerate() {
        if kind.id.is_empty() {
            return Err(CatalogError::EmptyId {
                record: Record::Kind,
            });
        }
        if ids.insert(kind.id.as_str(), index).is_some() {
            return Err(CatalogError::Duplicate {
                record: Record::Kind,
                id: kind.id.clone(),
            });
        }
    }
    let mut differences = Vec::with_capacity(declarations.len());
    for kind in declarations {
        let difference = match &kind.difference_kind {
            None => None,
            Some(id) => {
                let index = *ids.get(id.as_str()).ok_or_else(|| CatalogError::Unknown {
                    record: Record::Kind,
                    id: id.clone(),
                })?;
                let target = &declarations[index];
                if kind.dimensions != target.dimensions {
                    return Err(CatalogError::AffineDimensionMismatch {
                        point: kind.id.clone(),
                        difference: id.clone(),
                    });
                }
                if target.difference_kind.is_some() {
                    return Err(CatalogError::NestedAffineSpace {
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
        .iter()
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
            id: kind.id.clone(),
            dimensions: kind.dimensions.clone(),
            difference,
            canonical: None,
            minimum: None,
        })
        .collect())
}

fn check_magnitudes(unit: &UnitDecl) -> Result<(), CatalogError> {
    let zero_scale = || CatalogError::ZeroScale {
        unit: unit.id.clone(),
    };
    let nonfinite = || CatalogError::NonFiniteConversion {
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
