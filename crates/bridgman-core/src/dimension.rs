use std::collections::BTreeMap;
use std::fmt;
use std::ops::{Div, Mul};

use num_rational::{BigRational, ParseRatioError};
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dimensions(BTreeMap<String, BigRational>);

impl Serialize for Dimensions {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0
            .iter()
            .map(|(id, power)| (id, power.to_string()))
            .collect::<BTreeMap<_, _>>()
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Dimensions {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Power {
            Text(String),
            Integer(i64),
        }
        let values = BTreeMap::<String, Power>::deserialize(deserializer)?;
        let mut powers = Vec::with_capacity(values.len());
        for (id, power) in values {
            let power = match power {
                Power::Text(text) => parse_power(text).map_err(serde::de::Error::custom)?,
                Power::Integer(n) => BigRational::from_integer(n.into()),
            };
            powers.push((id, power));
        }
        Ok(Self::from_rational_powers(powers))
    }
}

#[derive(Clone, Debug, PartialEq, Error)]
pub enum DimensionError {
    #[error("invalid dimension signature component {0:?}")]
    InvalidSignature(String),
    #[error("invalid dimension exponent {text:?}")]
    InvalidPower {
        text: String,
        #[source]
        source: ParseRatioError,
    },
}

fn parse_power(text: String) -> Result<BigRational, DimensionError> {
    text.parse()
        .map_err(|source| DimensionError::InvalidPower { text, source })
}

const SIGNATURE_ORDER: [&str; 7] = ["M", "L", "T", "I", "Theta", "N", "J"];

impl Dimensions {
    pub fn one() -> Self {
        Self::default()
    }

    pub fn from_integer_powers<I, S>(powers: I) -> Self
    where
        I: IntoIterator<Item = (S, i64)>,
        S: Into<String>,
    {
        Self::from_rational_powers(
            powers
                .into_iter()
                .map(|(id, power)| (id, BigRational::from_integer(power.into()))),
        )
    }

    pub fn from_rational_powers<I, S>(powers: I) -> Self
    where
        I: IntoIterator<Item = (S, BigRational)>,
        S: Into<String>,
    {
        let mut result = Self::default();
        for (id, power) in powers {
            result.insert(id.into(), power);
        }
        result
    }

    fn insert(&mut self, id: String, power: BigRational) {
        let id = canonical_id(&id).to_owned();
        let new = self.0.get(&id).cloned().unwrap_or_else(BigRational::zero) + power;
        if new.is_zero() {
            self.0.remove(&id);
        } else {
            self.0.insert(id, new);
        }
    }

    /// Nonzero powers in signature order: the SI base dimensions first, then
    /// any other identifier lexicographically.
    pub fn powers(&self) -> impl Iterator<Item = (&str, &BigRational)> {
        let mut powers: Vec<_> = self
            .0
            .iter()
            .map(|(id, value)| (id.as_str(), value))
            .collect();
        powers.sort_by_key(|(id, _)| {
            (
                SIGNATURE_ORDER
                    .iter()
                    .position(|base| base == id)
                    .unwrap_or(SIGNATURE_ORDER.len()),
                *id,
            )
        });
        powers.into_iter()
    }

    pub fn pow(&self, power: &BigRational) -> Self {
        let mut result = Self::default();
        for (id, value) in &self.0 {
            result.insert(id.clone(), value * power);
        }
        result
    }

    pub fn signature(&self) -> String {
        if self.0.is_empty() {
            return "1".into();
        }
        self.powers()
            .map(|(id, v)| {
                if v.denom().is_one() {
                    format!("{id}:{}", v.numer())
                } else {
                    format!("{id}:{}/{}", v.numer(), v.denom())
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn parse_signature(value: &str) -> Result<Self, DimensionError> {
        if value == "1" {
            return Ok(Self::one());
        }
        let mut result = Self::default();
        for part in value.split(',') {
            let (id, power) = part
                .rsplit_once(':')
                .ok_or_else(|| DimensionError::InvalidSignature(part.into()))?;
            result.insert(id.into(), parse_power(power.into())?);
        }
        Ok(result)
    }
}

fn canonical_id(id: &str) -> &str {
    match id {
        "Θ" | "θ" => "Theta",
        other => other,
    }
}

impl Mul for &Dimensions {
    type Output = Dimensions;
    fn mul(self, rhs: Self) -> Self::Output {
        let mut result = self.clone();
        for (id, power) in &rhs.0 {
            result.insert(id.clone(), power.clone());
        }
        result
    }
}

impl Div for &Dimensions {
    type Output = Dimensions;
    fn div(self, rhs: Self) -> Self::Output {
        let mut result = self.clone();
        for (id, power) in &rhs.0 {
            result.insert(id.clone(), -power);
        }
        result
    }
}

impl fmt::Display for Dimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.signature())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_dimensions_are_exact_and_canonical() {
        let dimensions: Dimensions =
            serde_json::from_str(r#"{"Theta":"1/2","θ":"1/2","L":0}"#).unwrap();
        assert_eq!(dimensions, Dimensions::from_integer_powers([("Theta", 1)]));
        assert_eq!(
            serde_json::to_string(&dimensions).unwrap(),
            r#"{"Theta":"1"}"#
        );
        assert!(serde_json::from_str::<Dimensions>(r#"{"L":"1/0"}"#).is_err());
    }
    #[test]
    fn preserves_unknown_dimensions_and_exact_roots() {
        let dims = Dimensions::from_integer_powers([("user:q", 1), ("L", 2)]);
        let root = dims.pow(&BigRational::new(1.into(), 2.into()));
        assert_eq!(root.signature(), "L:1,user:q:1/2");
        assert_eq!(
            Dimensions::parse_signature(&root.signature()).unwrap(),
            root
        );
    }
    #[test]
    fn signature_orders_si_bases_first() {
        let dims = Dimensions::from_integer_powers([("J", 1), ("Theta", 1), ("M", 1), ("A", 1)]);
        assert_eq!(dims.signature(), "M:1,Theta:1,J:1,A:1");
    }
    #[test]
    fn canonicalizes_theta_aliases() {
        assert_eq!(
            Dimensions::from_integer_powers([("Θ", 1), ("Theta", -1)]),
            Dimensions::one()
        );
    }
    #[test]
    fn zero_denominator_signature_is_an_invalid_power() {
        assert!(matches!(
            Dimensions::parse_signature("L:1/0"),
            Err(DimensionError::InvalidPower { .. })
        ));
    }
}
