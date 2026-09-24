use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    Catalog, CatalogError, Conversion, Dimensions, ExactScalar, ExactValue, Grade, KindDecl,
    Magnitude, UnitDecl, CATALOG_SCHEMA,
};

// Only the fields the adapter reads are declared. The producer's other
// sections (declarations, diagnostics, numbers, factors, unresolved
// dependencies) are accepted and ignored.
#[derive(Deserialize)]
struct SourceCatalog {
    schema_version: u32,
    source: Source,
    resolved: Resolved,
    #[serde(default)]
    applied_corrections: serde_yaml::Value,
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
}
#[derive(Deserialize)]
struct SourceKind {
    dimensions: Option<Dimensions>,
}
#[derive(Deserialize)]
struct SourceUnit {
    #[serde(default)]
    name: String,
    /// An empty symbol is the producer's older spelling of an absent one.
    #[serde(default)]
    symbol: Option<String>,
    quantity_kinds: Vec<String>,
    #[serde(default)]
    si_factor: Option<ExactRecord>,
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
        rational: ExactScalar,
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
pub fn qudv_schema2_to_catalog(input: &str) -> Result<Catalog, CatalogError> {
    let source: SourceCatalog =
        serde_yaml::from_str(input).map_err(|error| CatalogError::QudvDocument(error.into()))?;
    if source.schema_version != 2 {
        return Err(CatalogError::Schema {
            expected: 2,
            actual: source.schema_version,
        });
    }
    if source.source.sha256.is_empty() {
        return Err(CatalogError::EmptySourceHash);
    }
    let scope = |id: &str| format!("qudv:{}:{id}", source.source.sha256);
    let kinds = source
        .resolved
        .kinds
        .into_iter()
        .map(|(id, kind)| KindDecl {
            id: scope(&id),
            dimensions: kind.dimensions,
            grade: Grade::Scalar,
            difference_kind: None,
            minimum: None,
        })
        .collect();
    let mut units = Vec::new();
    for (id, unit) in source.resolved.units {
        let coherent_scale = match unit.si_factor {
            Some(record) => match magnitude(record, &id)? {
                Magnitude::Exact(value) => value.monomial(),
                Magnitude::Approximate(_) => None,
            },
            None => None,
        };
        let conversion = match unit.conversion {
            Some(conversion) => Some(Conversion {
                reference_unit: scope(&conversion.reference_unit),
                scale: match magnitude(conversion.scale, &id)? {
                    Magnitude::Exact(value) => Magnitude::Exact(
                        value
                            .monomial()
                            .ok_or_else(|| CatalogError::NonMonomialScale { unit: id.clone() })?,
                    ),
                    Magnitude::Approximate(value) => Magnitude::Approximate(value),
                },
                offset: magnitude(conversion.offset, &id)?,
            }),
            None => None,
        };
        units.push(UnitDecl {
            id: scope(&id),
            symbol: unit
                .symbol
                .filter(|symbol| !symbol.is_empty())
                .unwrap_or(unit.name),
            kinds: unit.quantity_kinds.iter().map(|kind| scope(kind)).collect(),
            conversion,
            coherent_scale,
        });
    }
    let mut provenance = BTreeMap::new();
    provenance.insert("adapter".into(), "qudv-iso80000/schema-2".into());
    provenance.insert("source_sha256".into(), source.source.sha256);
    provenance.insert(
        "applied_corrections".into(),
        serde_json::to_string(&source.applied_corrections)
            .map_err(|error| CatalogError::ProvenanceEncoding(error.into()))?,
    );
    if !source.source.format.is_empty() {
        provenance.insert("source_format".into(), source.source.format);
    }
    Ok(Catalog {
        schema: CATALOG_SCHEMA,
        provenance,
        dimensionless: None,
        kinds,
        units,
        operations: vec![],
    })
}

fn magnitude(record: ExactRecord, unit: &str) -> Result<Magnitude<ExactValue>, CatalogError> {
    match record {
        ExactRecord::Term {
            mut rational,
            pi_exponent,
        } => {
            rational.pi_exponent += pi_exponent;
            Ok(Magnitude::Exact(ExactValue::from_scalar(rational)))
        }
        ExactRecord::Sum { sum } => {
            let mut total = ExactValue::default();
            for record in sum {
                match magnitude(record, unit)? {
                    Magnitude::Exact(value) => total = total.add(&value),
                    Magnitude::Approximate(_) => {
                        return Err(CatalogError::MixedApproximateSum { unit: unit.into() })
                    }
                }
            }
            Ok(Magnitude::Exact(total))
        }
        ExactRecord::Approximate { approximate } => Ok(Magnitude::Approximate(approximate)),
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
    celsius:
      name: degree Celsius
      symbol: ''
      quantity_kinds: [temperature]
      conversion: {reference_unit: kelvin, scale: {rational: '1'}, offset: {sum: [{rational: '273'}, {rational: '3/20'}]}}
    unresolved:
      name: unresolved
      symbol: null
      quantity_kinds: [generalized]
      conversion: null
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
            r.kind("qudv:abc:generalized").unwrap().dimensions(),
            Err(crate::QuantityError::UnresolvedDimensions(
                "qudv:abc:generalized".into()
            ))
        );
    }
    #[test]
    fn empty_symbol_is_absent_and_sums_are_exact() {
        let c = qudv_schema2_to_catalog(FIXTURE).unwrap();
        let celsius = c.units.iter().find(|u| u.id == "qudv:abc:celsius").unwrap();
        assert_eq!(celsius.symbol, "degree Celsius");
        assert_eq!(
            celsius.conversion.as_ref().unwrap().offset,
            Magnitude::Exact(ExactValue::from_scalar(
                ExactScalar::parse("5463/20").unwrap()
            ))
        );
        let unresolved = c
            .units
            .iter()
            .find(|u| u.id == "qudv:abc:unresolved")
            .unwrap();
        assert_eq!(unresolved.symbol, "unresolved");
        assert!(unresolved.conversion.is_none());
    }
    #[test]
    fn invalid_exponents_are_document_errors() {
        let bad = FIXTURE.replace("{Theta: 1}", "{Theta: '1/0'}");
        assert!(matches!(
            qudv_schema2_to_catalog(&bad),
            Err(CatalogError::QudvDocument(_))
        ));
    }
}
