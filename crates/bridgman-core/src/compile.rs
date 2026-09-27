//! Checking a catalog's declarations and compiling them into a `Registry`.
//! Every refusal here is a `CatalogError`: a fault of the declarations, found
//! before any quantity exists.
use crate::catalog::{Catalog, KindDecl, Magnitude, ProductOp, UnitDecl, CATALOG_SCHEMA};
use crate::derive::{resolve, Resolved};
use crate::registry::{CompiledConversion, CompiledKind, CompiledUnit, Minimum, TwinRow};
use crate::{
    AffineRole, CatalogError, DerivationError, Dimensions, Grade, RateFault, Record, Registry,
};
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
                .ok_or_else(|| unknown(Record::Kind, id))
        };
        let mut unit_ids = HashMap::new();
        let mut symbols: HashMap<String, Vec<usize>> = HashMap::new();
        let mut unit_kinds = Vec::with_capacity(catalog.units.len());
        let terminal: Vec<bool> = catalog.units.iter().map(UnitDecl::is_terminal).collect();
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
            let mut indexes = Vec::with_capacity(unit.kinds.len());
            for kind in &unit.kinds {
                let kind = known_kind(kind)?;
                if terminal[index] && kinds[kind].canonical.is_none() {
                    kinds[kind].canonical = Some(index);
                }
                indexes.push(kind);
            }
            unit_kinds.push(indexes);
            symbols.entry(unit.symbol.clone()).or_default().push(index);
        }
        let mut units = Vec::with_capacity(catalog.units.len());
        for (unit, unit_kinds_of) in catalog.units.into_iter().zip(&unit_kinds) {
            let conversion = match unit.conversion {
                None => None,
                Some(conversion) => {
                    let reference = &conversion.reference_unit;
                    let index = *unit_ids
                        .get(reference)
                        .ok_or_else(|| unknown(Record::Unit, reference))?;
                    for &kind in unit_kinds_of {
                        if let Some(dimensions) = &kinds[kind].dimensions {
                            if !unit_kinds[index].iter().any(|&target| {
                                kinds[target].dimensions.as_ref() == Some(dimensions)
                            }) {
                                return Err(CatalogError::IncompatibleReference {
                                    unit: unit.id.clone(),
                                    reference: reference.clone(),
                                    kind: kinds[kind].id.clone(),
                                });
                            }
                        }
                    }
                    if !terminal[index] {
                        return Err(CatalogError::NonIdentityReference {
                            unit: unit.id.clone(),
                            reference: reference.clone(),
                        });
                    }
                    Some(CompiledConversion {
                        reference: index,
                        scale: conversion.scale,
                        offset: conversion.offset,
                    })
                }
            };
            units.push(CompiledUnit {
                id: unit.id,
                symbol: unit.symbol,
                kinds: unit_kinds_of.clone(),
                conversion,
                coherent_scale: unit.coherent_scale,
            });
        }
        // A least value is stated in the canonical unit, so every unit of the
        // kind must reach that unit.
        for (index, (declaration, kind)) in catalog.kinds.iter().zip(&mut kinds).enumerate() {
            let Some(minimum) = &declaration.minimum else {
                continue;
            };
            let invalid = || CatalogError::InvalidMinimum {
                kind: kind.id.clone(),
            };
            let canonical = kind.canonical.ok_or_else(invalid)?;
            let off_canonical = units.iter().find(|unit| {
                unit.kinds.contains(&index)
                    && unit
                        .conversion
                        .as_ref()
                        .is_none_or(|c| c.reference != canonical)
            });
            if off_canonical.is_some() {
                return Err(invalid());
            }
            kind.minimum = Some(Minimum {
                declared: minimum.clone(),
                value: minimum.to_f64().ok_or_else(invalid)?,
                unit: canonical,
            });
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
        let time = match &catalog.time {
            None => None,
            Some(id) => {
                let index = known_kind(id)?;
                let kind = &kinds[index];
                if kind.role != AffineRole::Point || kind.grade != Grade::Scalar {
                    return Err(CatalogError::InvalidTimeKind(id.clone()));
                }
                Some(index)
            }
        };
        let duration = time.and_then(|index| kinds[index].difference);
        // A rate times a duration is what it is the rate of; each kind has at
        // most one rate.
        for (index, declaration) in catalog.kinds.iter().enumerate() {
            let Some(of_id) = &declaration.rate_of else {
                continue;
            };
            let of = known_kind(of_id)?;
            let fault = |fault| CatalogError::InvalidRate {
                rate: declaration.id.clone(),
                of: of_id.clone(),
                fault,
            };
            let Some(duration) = duration else {
                return Err(fault(RateFault::NoTimeKind));
            };
            for kind in [index, of] {
                if kinds[kind].role == AffineRole::Point {
                    return Err(fault(RateFault::PointKind(kinds[kind].id.clone())));
                }
            }
            let dimensions = |i: usize| {
                kinds[i].dimensions.as_ref().ok_or_else(|| {
                    CatalogError::Derivation(DerivationError::UnresolvedDimensions {
                        kind: kinds[i].id.clone(),
                    })
                })
            };
            let (rate, target, over) = (dimensions(index)?, dimensions(of)?, dimensions(duration)?);
            let product = rate * over;
            let grade = kinds[index].grade;
            if product != *target || grade != kinds[of].grade {
                return Err(fault(RateFault::Mismatch {
                    dimensions: product,
                    grade,
                }));
            }
            if let Some(existing) = kinds[of].rate {
                return Err(fault(RateFault::AlsoRateOf(kinds[existing].id.clone())));
            }
            kinds[index].rate_of = Some(of);
            kinds[of].rate = Some(index);
        }
        // A row is kept exactly when derivation leaves two or more candidates
        // and the row names one of them.
        let mut twins = HashMap::new();
        for operation in &catalog.operations {
            let op = operation.op;
            if operation.commutative && op == ProductOp::Div {
                return Err(CatalogError::CommutativeQuotient {
                    left: operation.left.clone(),
                    right: operation.right.clone(),
                });
            }
            let left = known_kind(&operation.left)?;
            let right = known_kind(&operation.right)?;
            let result = known_kind(&operation.result)?;
            let named = |error: DerivationError<usize>| {
                CatalogError::Derivation(error.map_kinds(|kind| kinds[kind].id.clone()))
            };
            if kinds[result].role == AffineRole::Point {
                return Err(named(DerivationError::Point {
                    left,
                    op,
                    right,
                    point: result,
                }));
            }
            let Some(result_dimensions) = &kinds[result].dimensions else {
                return Err(named(DerivationError::UnresolvedDimensions {
                    kind: result,
                }));
            };
            let derivation =
                resolve(&kinds, dimensionless, duration, left, op, right).map_err(named)?;
            let invalid = || CatalogError::InvalidOperationRule {
                left: operation.left.clone(),
                op,
                right: operation.right.clone(),
                result: operation.result.clone(),
                dimensions: derivation.dimensions.clone(),
                grade: derivation.grade,
            };
            if *result_dimensions != derivation.dimensions
                || kinds[result].grade != derivation.grade
            {
                return Err(invalid());
            }
            match &derivation.resolved {
                Resolved::Kind(derived) => {
                    return Err(CatalogError::DerivedOperationRule {
                        left: operation.left.clone(),
                        op,
                        right: operation.right.clone(),
                        result: operation.result.clone(),
                        derived: kinds[*derived].id.clone(),
                    })
                }
                Resolved::Twins(_) => {}
                Resolved::None => return Err(invalid()),
            }
            let mut insert = |left: usize, right: usize| {
                let row = TwinRow {
                    result,
                    provenance: operation.provenance.clone(),
                };
                if twins.insert((left, op, right), row).is_some() {
                    Err(CatalogError::ConflictingOperationRule {
                        left: kinds[left].id.clone(),
                        op,
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
            units,
            kind_ids,
            unit_ids,
            symbols,
            twins,
            dimensionless,
            time,
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
                let index = *ids
                    .get(id.as_str())
                    .ok_or_else(|| unknown(Record::Kind, id))?;
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
            grade: kind.grade,
            difference,
            canonical: None,
            minimum: None,
            rate_of: None,
            rate: None,
        })
        .collect())
}

fn unknown(record: Record, id: &str) -> CatalogError {
    CatalogError::Derivation(DerivationError::Unknown {
        record,
        id: id.into(),
    })
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
