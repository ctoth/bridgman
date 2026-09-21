use std::collections::BTreeMap;

use num_bigint::BigInt;
use serde::Deserialize;

use crate::{Catalog, Dimensions, ExactScalar, KindDecl, QuantityError, UnitDecl, CATALOG_SCHEMA};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceCatalog {
    schema_version: u32,
    source: Source,
    #[serde(default, rename = "declarations")]
    _declarations: BTreeMap<String, serde_yaml::Value>,
    resolved: Resolved,
    #[serde(default)]
    #[serde(rename = "diagnostics")]
    _diagnostics: serde_yaml::Value,
    #[serde(default, rename = "applied_corrections")]
    _applied_corrections: serde_yaml::Value,
}
#[derive(Deserialize)]
struct Source {
    sha256: String,
    #[serde(default)]
    format: String,
}
#[derive(Deserialize)]
struct Resolved {
    kinds: BTreeMap<String, SourceKind>,
    units: BTreeMap<String, SourceUnit>,
    #[serde(default)]
    #[serde(rename = "numbers")]
    _numbers: serde_yaml::Value,
    #[serde(default, rename = "factors")]
    _factors: serde_yaml::Value,
}
#[derive(Deserialize)]
struct SourceKind {
    dimensions: Option<BTreeMap<String, serde_yaml::Value>>,
    #[serde(default)]
    #[serde(rename = "factors")]
    _factors: serde_yaml::Value,
    #[serde(default, rename = "unresolved_dependencies")]
    _unresolved_dependencies: Vec<String>,
}
#[derive(Deserialize)]
struct SourceUnit {
    #[serde(default)]
    name: String,
    #[serde(default)]
    symbol: Option<String>,
    quantity_kinds: Vec<String>,
    #[serde(default)]
    #[serde(rename = "si_factor")]
    _si_factor: serde_yaml::Value,
    conversion: Option<SourceConversion>,
}
#[derive(Deserialize)]
struct SourceConversion {
    reference_unit: String,
    scale: ExactRecord,
    offset: ExactRecord,
}
#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum ExactRecord {
    Term {
        rational: String,
        #[serde(default)]
        pi_exponent: i32,
    },
    Sum {
        sum: Vec<ExactRecord>,
    },
    Approximate {
        approximate: f64,
    },
}

/// Adapt a source-preserving QUDV schema-2 document without inferring aliases,
/// kinds, or operations. Every imported ID is scoped by the source hash.
pub fn qudv_schema2_to_catalog(input: &str) -> Result<Catalog, QuantityError> {
    let source: SourceCatalog =
        serde_yaml::from_str(input).map_err(|e| QuantityError::InvalidCatalog(e.to_string()))?;
    if source.schema_version != 2 {
        return Err(QuantityError::Schema {
            expected: 2,
            actual: source.schema_version,
        });
    }
    if source.source.sha256.is_empty() {
        return Err(QuantityError::InvalidCatalog(
            "QUDV source hash is empty".into(),
        ));
    }
    let scope = |id: &str| format!("qudv:{}:{id}", source.source.sha256);
    let mut kinds = Vec::new();
    for (id, kind) in source.resolved.kinds {
        kinds.push(KindDecl {
            id: scope(&id),
            dimensions: kind.dimensions.map(parse_dimensions).transpose()?,
            difference_kind: None,
        });
    }
    let mut units = Vec::new();
    for (id, unit) in source.resolved.units {
        let (reference_unit, scale, approximate_scale, offset, offset_terms, approximate_offset) =
            if let Some(conversion) = unit.conversion {
                let (scale_terms, approximate_scale) = scalar_terms(conversion.scale)?;
                if scale_terms.len() > 1 {
                    return Err(QuantityError::InvalidCatalog(format!(
                        "unit {id:?} has a non-monomial scale"
                    )));
                }
                let (offset_terms, approximate_offset) = scalar_terms(conversion.offset)?;
                let offset = offset_terms
                    .first()
                    .cloned()
                    .unwrap_or_else(ExactScalar::zero);
                (
                    Some(scope(&conversion.reference_unit)),
                    scale_terms.first().cloned(),
                    approximate_scale,
                    offset,
                    offset_terms,
                    approximate_offset,
                )
            } else {
                (None, None, None, ExactScalar::zero(), vec![], None)
            };
        units.push(UnitDecl {
            id: scope(&id),
            symbol: unit.symbol.unwrap_or(unit.name),
            kinds: unit.quantity_kinds.iter().map(|kind| scope(kind)).collect(),
            reference_unit,
            scale,
            approximate_scale,
            offset,
            offset_terms,
            approximate_offset,
        });
    }
    let mut provenance = BTreeMap::new();
    provenance.insert("adapter".into(), "qudv-iso80000/schema-2".into());
    provenance.insert("source_sha256".into(), source.source.sha256);
    if !source.source.format.is_empty() {
        provenance.insert("source_format".into(), source.source.format);
    }
    Ok(Catalog {
        schema: CATALOG_SCHEMA,
        provenance,
        kinds,
        units,
        operations: vec![],
    })
}

fn parse_dimensions(
    values: BTreeMap<String, serde_yaml::Value>,
) -> Result<Dimensions, QuantityError> {
    let mut powers = Vec::new();
    for (id, value) in values {
        let text = match value {
            serde_yaml::Value::Number(n) => n.to_string(),
            serde_yaml::Value::String(s) => s,
            _ => {
                return Err(QuantityError::InvalidCatalog(format!(
                    "invalid exponent for dimension {id:?}"
                )))
            }
        };
        let (n, d) = text.split_once('/').unwrap_or((&text, "1"));
        let n = n
            .parse::<BigInt>()
            .map_err(|_| QuantityError::InvalidCatalog(format!("invalid exponent {text:?}")))?;
        let d = d
            .parse::<BigInt>()
            .map_err(|_| QuantityError::InvalidCatalog(format!("invalid exponent {text:?}")))?;
        powers.push((id, (n, d)));
    }
    Dimensions::from_rational_powers(powers)
        .map_err(|e| QuantityError::InvalidCatalog(e.to_string()))
}
fn scalar_terms(record: ExactRecord) -> Result<(Vec<ExactScalar>, Option<f64>), QuantityError> {
    match record {
        ExactRecord::Term {
            rational,
            pi_exponent,
        } => {
            let mut value = ExactScalar::parse(&rational)
                .map_err(|e| QuantityError::InvalidCatalog(e.to_string()))?;
            value.pi_exponent += pi_exponent;
            Ok((vec![value], None))
        }
        ExactRecord::Sum { sum } => {
            let mut result = Vec::new();
            for value in sum {
                let (terms, approximate) = scalar_terms(value)?;
                if approximate.is_some() {
                    return Err(QuantityError::InvalidCatalog(
                        "mixed approximate offset sum is unsupported".into(),
                    ));
                }
                result.extend(terms);
            }
            Ok((result, None))
        }
        ExactRecord::Approximate { approximate } => Ok((vec![], Some(approximate))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = r#"
schema_version: 2
source: {sha256: abc, format: OMG XMI}
declarations: {}
resolved:
  kinds:
    temperature: {dimensions: {Theta: 1}}
    generalized: {dimensions: null, unresolved_dependencies: [coordinate]}
  units:
    kelvin:
      name: kelvin
      symbol: K
      quantity_kinds: [temperature]
      si_factor: {rational: '1', pi_exponent: 0}
      conversion: {reference_unit: kelvin, scale: {rational: '1', pi_exponent: 0}, offset: {rational: '0', pi_exponent: 0}}
diagnostics: {}
applied_corrections: null
"#;
    #[test]
    fn scopes_source_ids_and_preserves_unresolved_dimensions() {
        let c = qudv_schema2_to_catalog(FIXTURE).unwrap();
        assert!(c
            .kinds
            .iter()
            .any(|k| k.id == "qudv:abc:generalized" && k.dimensions.is_none()));
        let r = crate::Registry::compile(c).unwrap();
        assert_eq!(
            r.dimensions(r.kind("qudv:abc:generalized").unwrap()),
            Err(QuantityError::UnresolvedDimensions(
                "qudv:abc:generalized".into()
            ))
        );
    }
}
