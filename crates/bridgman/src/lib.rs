//! Arbitrary-precision dimensional arithmetic.
//!
//! Dimensions are insertion-ordered maps from dimension names to integer
//! exponents. Arithmetic preserves that order while canonical signatures use
//! the SI base-dimension order.

use std::fmt;

use indexmap::IndexMap;
use indexmap::map::Entry;
use num_bigint::BigInt;

/// A dimension map with arbitrary-precision integer exponents.
pub type Dimensions = IndexMap<String, BigInt>;

/// Canonical SI base-dimension display order.
pub const DIMENSION_ORDER: [&str; 7] = ["M", "L", "T", "I", "Theta", "N", "J"];

/// Remove entries whose exponent is zero.
#[must_use]
pub fn clean(dimensions: &Dimensions) -> Dimensions {
    let mut result = dimensions.clone();
    result.retain(|_, exponent| exponent != &BigInt::from(0));
    result
}

/// Normalize temperature keys and combine equivalent entries.
#[must_use]
pub fn canonicalize(dimensions: &Dimensions) -> Dimensions {
    let mut result = Dimensions::new();
    for (key, exponent) in dimensions {
        let canonical_key = canonical_key(key);
        match result.entry(canonical_key.to_owned()) {
            Entry::Occupied(mut entry) => *entry.get_mut() += exponent,
            Entry::Vacant(entry) => {
                entry.insert(exponent.clone());
            }
        }
    }
    clean(&result)
}

/// Multiply quantities by adding their dimension exponents.
#[must_use]
pub fn multiply(left: &Dimensions, right: &Dimensions) -> Dimensions {
    combine(left, right, false)
}

/// Divide quantities by subtracting the divisor's dimension exponents.
#[must_use]
pub fn divide(left: &Dimensions, right: &Dimensions) -> Dimensions {
    combine(left, right, true)
}

/// Raise dimensions to an arbitrary-precision integer power.
#[must_use]
pub fn power(dimensions: &Dimensions, exponent: &BigInt) -> Dimensions {
    if exponent == &BigInt::from(0) {
        return Dimensions::new();
    }
    dimensions
        .iter()
        .filter_map(|(key, value)| {
            let product = value * exponent;
            (product != BigInt::from(0)).then(|| (key.clone(), product))
        })
        .collect()
}

/// Compare dimensions after stripping zero exponents.
#[must_use]
pub fn equal(left: &Dimensions, right: &Dimensions) -> bool {
    let left = clean(left);
    let right = clean(right);
    left.len() == right.len()
        && left
            .iter()
            .all(|(key, exponent)| right.get(key) == Some(exponent))
}

/// Return whether no nonzero dimension exponents remain.
#[must_use]
pub fn is_dimensionless(dimensions: &Dimensions) -> bool {
    dimensions
        .values()
        .all(|exponent| exponent == &BigInt::from(0))
}

/// Format dimensions with Unicode superscript exponents.
#[must_use]
pub fn format(dimensions: &Dimensions) -> String {
    let cleaned = clean(dimensions);
    if cleaned.is_empty() {
        return "1".to_owned();
    }

    let mut parts: Vec<_> = cleaned.iter().collect();
    parts.sort_by_key(|(key, _)| dimension_index(key));
    parts
        .into_iter()
        .map(|(key, exponent)| {
            if exponent == &BigInt::from(1) {
                key.clone()
            } else {
                format!("{key}{}", superscript(exponent))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Produce a canonical, zero-stripped signature.
#[must_use]
pub fn signature(dimensions: &Dimensions) -> String {
    let canonical = canonicalize(dimensions);
    if canonical.is_empty() {
        return "1".to_owned();
    }

    let mut parts: Vec<_> = canonical.iter().collect();
    parts.sort_by(|(left_key, _), (right_key, _)| {
        dimension_index(left_key)
            .cmp(&dimension_index(right_key))
            .then_with(|| left_key.cmp(right_key))
    });
    parts
        .into_iter()
        .map(|(key, exponent)| format!("{key}:{exponent}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Parse a signature produced by [`signature`].
pub fn parse_signature(value: &str) -> Result<Dimensions, ParseDimensionsError> {
    if value == "1" {
        return Ok(Dimensions::new());
    }

    let mut parsed = Dimensions::new();
    for part in value.split(',') {
        let (key, exponent) = part
            .split_once(':')
            .ok_or_else(|| ParseDimensionsError::new(format!("missing ':' in {part:?}")))?;
        let exponent = exponent.parse::<BigInt>().map_err(|_| {
            ParseDimensionsError::new(format!("invalid integer exponent {exponent:?}"))
        })?;
        parsed.insert(key.to_owned(), exponent);
    }
    Ok(canonicalize(&parsed))
}

/// An invalid canonical dimension signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseDimensionsError {
    message: String,
}

impl ParseDimensionsError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for ParseDimensionsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ParseDimensionsError {}

fn canonical_key(key: &str) -> &str {
    match key {
        "\u{0398}" | "\u{03b8}" | "Theta" => "Theta",
        _ => key,
    }
}

fn combine(left: &Dimensions, right: &Dimensions, subtract: bool) -> Dimensions {
    let mut result = left.clone();
    for (key, exponent) in right {
        let value = if subtract {
            -exponent
        } else {
            exponent.clone()
        };
        match result.entry(key.clone()) {
            Entry::Occupied(mut entry) => *entry.get_mut() += value,
            Entry::Vacant(entry) => {
                entry.insert(value);
            }
        }
    }
    clean(&result)
}

fn dimension_index(key: &str) -> usize {
    DIMENSION_ORDER
        .iter()
        .position(|candidate| candidate == &key)
        .unwrap_or(DIMENSION_ORDER.len())
}

fn superscript(exponent: &BigInt) -> String {
    exponent
        .to_string()
        .chars()
        .map(|character| match character {
            '-' => '\u{207b}',
            '0' => '\u{2070}',
            '1' => '\u{00b9}',
            '2' => '\u{00b2}',
            '3' => '\u{00b3}',
            '4' => '\u{2074}',
            '5' => '\u{2075}',
            '6' => '\u{2076}',
            '7' => '\u{2077}',
            '8' => '\u{2078}',
            '9' => '\u{2079}',
            _ => character,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dimensions(entries: &[(&str, &str)]) -> Dimensions {
        entries
            .iter()
            .map(|(key, exponent)| {
                (
                    (*key).to_owned(),
                    exponent.parse().expect("test exponent must be valid"),
                )
            })
            .collect()
    }

    #[test]
    fn arithmetic_uses_arbitrary_precision_exponents() {
        let huge = "100000000000000000000000000000000000000000000000000";
        let left = dimensions(&[("L", huge), ("T", "-2")]);
        let right = dimensions(&[("L", huge), ("T", "2")]);
        let product = multiply(&left, &right);

        assert_eq!(product.len(), 1);
        assert_eq!(
            product["L"],
            huge.parse::<BigInt>().expect("valid integer") * 2
        );
    }

    #[test]
    fn canonicalization_combines_theta_and_strips_zeroes() {
        let input = dimensions(&[("\u{0398}", "4"), ("Theta", "-4"), ("L", "0")]);
        assert!(canonicalize(&input).is_empty());
    }

    #[test]
    fn signatures_and_formatting_are_canonical() {
        let input = dimensions(&[("T", "-2"), ("custom", "3"), ("M", "1"), ("L", "0")]);
        assert_eq!(signature(&input), "M:1,T:-2,custom:3");
        assert_eq!(format(&input), "M T\u{207b}\u{00b2} custom\u{00b3}");
        assert_eq!(
            parse_signature(&signature(&input)).expect("signature must parse"),
            canonicalize(&input)
        );
    }
}
