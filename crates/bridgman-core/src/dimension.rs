use std::collections::BTreeMap;
use std::fmt;
use std::ops::{Div, Mul};

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Dimensions(BTreeMap<String, BigRational>);

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum DimensionError {
    #[error("invalid dimension signature component {0:?}")]
    InvalidSignature(String),
    #[error("dimension exponent denominator cannot be zero")]
    ZeroDenominator,
}

impl Dimensions {
    pub fn one() -> Self {
        Self::default()
    }

    pub fn from_integer_powers<I, S>(powers: I) -> Self
    where
        I: IntoIterator<Item = (S, i64)>,
        S: Into<String>,
    {
        let mut result = Self::default();
        for (id, power) in powers {
            result.insert(id.into(), BigRational::from_integer(power.into()));
        }
        result
    }

    pub fn from_rational_powers<I, S>(powers: I) -> Result<Self, DimensionError>
    where
        I: IntoIterator<Item = (S, (BigInt, BigInt))>,
        S: Into<String>,
    {
        let mut result = Self::default();
        for (id, (n, d)) in powers {
            if d.is_zero() {
                return Err(DimensionError::ZeroDenominator);
            }
            result.insert(id.into(), BigRational::new(n, d));
        }
        Ok(result)
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

    pub fn powers(&self) -> impl Iterator<Item = (&str, &BigRational)> {
        self.0.iter().map(|(id, value)| (id.as_str(), value))
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
        self.0
            .iter()
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
            let (n, d) = power.split_once('/').unwrap_or((power, "1"));
            let n = n
                .parse::<BigInt>()
                .map_err(|_| DimensionError::InvalidSignature(part.into()))?;
            let d = d
                .parse::<BigInt>()
                .map_err(|_| DimensionError::InvalidSignature(part.into()))?;
            if d.is_zero() {
                return Err(DimensionError::ZeroDenominator);
            }
            result.insert(id.into(), BigRational::new(n, d));
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
    fn canonicalizes_theta_aliases() {
        assert_eq!(
            Dimensions::from_integer_powers([("Θ", 1), ("Theta", -1)]),
            Dimensions::one()
        );
    }
}
