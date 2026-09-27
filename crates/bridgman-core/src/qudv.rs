use std::collections::BTreeMap;

use num_bigint::BigInt;
use serde::{Deserialize, Deserializer};

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
    /// Absent when the source gives none; the unit is then written by name.
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
        /// `ExactScalar`'s own exponent type, so no exponent is narrowed.
        #[serde(default, deserialize_with = "pi_exponent")]
        pi_exponent: BigInt,
    },
    Sum {
        sum: Vec<ExactRecord>,
    },
    Approximate {
        approximate: f64,
    },
}

/// A pi exponent as the producer writes it: an integer, or integer text for
/// one wider than a machine integer. Anything else is a document error.
fn pi_exponent<'de, D: Deserializer<'de>>(deserializer: D) -> Result<BigInt, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Written {
        Integer(i64),
        Text(String),
    }
    match Written::deserialize(deserializer)? {
        Written::Integer(value) => Ok(value.into()),
        Written::Text(text) => text.parse().map_err(serde::de::Error::custom),
    }
}

/// Adapt a source-preserving QUDV schema-2 document without inferring aliases,
/// kinds, or operations. Every imported ID is scoped by the source hash.
pub fn qudv_schema2_to_catalog(input: &str) -> Result<Catalog, CatalogError> {
    let source: SourceCatalog =
        serde_yaml::from_str(input).map_err(|error| CatalogError::QudvDocument(error.into()))?;
    if source.schema_version != 2 {
        return Err(CatalogError::QudvSchema {
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
            rate_of: None,
        })
        .collect();
    let mut units = Vec::new();
    for (id, unit) in source.resolved.units {
        let monomial = |value: ExactValue| {
            value
                .monomial()
                .ok_or_else(|| CatalogError::NonMonomialScale { unit: id.clone() })
        };
        let coherent_scale = match unit.si_factor {
            Some(record) => match magnitude(record, &id)? {
                Magnitude::Exact(value) => Some(monomial(value)?),
                Magnitude::Approximate(_) => {
                    return Err(CatalogError::ApproximateSiFactor { unit: id.clone() })
                }
            },
            None => None,
        };
        let conversion = match unit.conversion {
            Some(conversion) => Some(Conversion {
                reference_unit: scope(&conversion.reference_unit),
                scale: match magnitude(conversion.scale, &id)? {
                    Magnitude::Exact(value) => Magnitude::Exact(monomial(value)?),
                    Magnitude::Approximate(value) => Magnitude::Approximate(value),
                },
                offset: magnitude(conversion.offset, &id)?,
            }),
            None => None,
        };
        let symbol = match unit.symbol {
            Some(symbol) if symbol.is_empty() => {
                return Err(CatalogError::EmptySymbol { unit: id.clone() })
            }
            Some(symbol) => symbol,
            None => unit.name,
        };
        units.push(UnitDecl {
            id: scope(&id),
            symbol,
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
        time: None,
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
      symbol: null
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
        let generalized = r.kind("qudv:abc:generalized").unwrap();
        assert_eq!(
            generalized.dimensions(),
            Err(crate::QuantityError::Derivation(
                crate::DerivationError::UnresolvedDimensions { kind: generalized }
            ))
        );
    }
    fn refused(document: &str) -> CatalogError {
        qudv_schema2_to_catalog(document).unwrap_err()
    }
    #[test]
    fn information_the_catalog_cannot_hold_is_refused() {
        let kelvin_factor = "si_factor: {rational: '1', pi_exponent: 0}";
        assert_eq!(
            refused(&FIXTURE.replace(kelvin_factor, "si_factor: {approximate: 1.0}")),
            CatalogError::ApproximateSiFactor {
                unit: "kelvin".into()
            }
        );
        assert_eq!(
            refused(&FIXTURE.replace(
                kelvin_factor,
                "si_factor: {sum: [{rational: '1'}, {rational: '1', pi_exponent: 1}]}"
            )),
            CatalogError::NonMonomialScale {
                unit: "kelvin".into()
            }
        );
        assert_eq!(
            refused(&FIXTURE.replace("symbol: K", "symbol: ''")),
            CatalogError::EmptySymbol {
                unit: "kelvin".into()
            }
        );
        // No symbol and no name leaves nothing to write the unit with.
        assert_eq!(
            refused(&FIXTURE.replace("      name: unresolved\n", "")),
            CatalogError::EmptySymbol {
                unit: "unresolved".into()
            }
        );
        assert_eq!(
            refused(&FIXTURE.replace("schema_version: 2", "schema_version: 3")),
            CatalogError::QudvSchema {
                expected: 2,
                actual: 3
            }
        );
    }
    #[test]
    fn pi_exponents_are_exact_integers() {
        let huge = FIXTURE.replace(
            "si_factor: {rational: '1', pi_exponent: 0}",
            "si_factor: {rational: '2', pi_exponent: 3000000000}",
        );
        let c = qudv_schema2_to_catalog(&huge).unwrap();
        let kelvin = c.units.iter().find(|u| u.id == "qudv:abc:kelvin").unwrap();
        assert_eq!(
            kelvin.coherent_scale,
            Some(ExactScalar::parse("2*pi^3000000000").unwrap())
        );
        let fractional = FIXTURE.replace(
            "si_factor: {rational: '1', pi_exponent: 0}",
            "si_factor: {rational: '1', pi_exponent: 0.5}",
        );
        assert!(matches!(
            refused(&fractional),
            CatalogError::QudvDocument(_)
        ));
    }
    #[test]
    fn absent_symbol_is_the_name_and_sums_are_exact() {
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
