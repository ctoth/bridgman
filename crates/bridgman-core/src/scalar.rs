use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactScalar {
    pub rational: BigRational,
    pub pi_exponent: i32,
}

#[derive(Debug, Error)]
pub enum ScalarError {
    #[error("invalid exact scalar {0:?}")]
    Invalid(String),
    #[error("exact scalar operation is unsupported when pi exponents differ")]
    UnlikePiPowers,
}

impl ExactScalar {
    pub fn one() -> Self {
        Self {
            rational: BigRational::one(),
            pi_exponent: 0,
        }
    }
    pub fn zero() -> Self {
        Self {
            rational: BigRational::zero(),
            pi_exponent: 0,
        }
    }
    pub fn parse(value: &str) -> Result<Self, ScalarError> {
        let (coefficient, pi_exponent) = match value.split_once("*pi^") {
            Some((c, p)) => (
                c,
                p.parse().map_err(|_| ScalarError::Invalid(value.into()))?,
            ),
            None => (value, 0),
        };
        let (n, d) = coefficient.split_once('/').unwrap_or((coefficient, "1"));
        let n = BigInt::from_str(n).map_err(|_| ScalarError::Invalid(value.into()))?;
        let d = BigInt::from_str(d).map_err(|_| ScalarError::Invalid(value.into()))?;
        if d.is_zero() {
            return Err(ScalarError::Invalid(value.into()));
        }
        Ok(Self {
            rational: BigRational::new(n, d),
            pi_exponent,
        })
    }
    pub fn multiply(&self, rhs: &Self) -> Self {
        Self {
            rational: &self.rational * &rhs.rational,
            pi_exponent: self.pi_exponent + rhs.pi_exponent,
        }
    }
    pub fn divide(&self, rhs: &Self) -> Option<Self> {
        if rhs.rational.is_zero() {
            None
        } else {
            Some(Self {
                rational: &self.rational / &rhs.rational,
                pi_exponent: self.pi_exponent - rhs.pi_exponent,
            })
        }
    }
    pub fn add(&self, rhs: &Self) -> Result<Self, ScalarError> {
        if self.pi_exponent != rhs.pi_exponent {
            return Err(ScalarError::UnlikePiPowers);
        }
        Ok(Self {
            rational: &self.rational + &rhs.rational,
            pi_exponent: self.pi_exponent,
        })
    }
    pub fn to_f64(&self) -> Option<f64> {
        let coefficient = self.rational.to_f64()?;
        let value = coefficient * std::f64::consts::PI.powi(self.pi_exponent);
        value.is_finite().then_some(value)
    }
    pub fn encoded(&self) -> String {
        let base = if self.rational.denom().is_one() {
            self.rational.numer().to_string()
        } else {
            format!("{}/{}", self.rational.numer(), self.rational.denom())
        };
        if self.pi_exponent == 0 {
            base
        } else {
            format!("{base}*pi^{}", self.pi_exponent)
        }
    }
}

impl Serialize for ExactScalar {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.encoded())
    }
}
impl<'de> Deserialize<'de> for ExactScalar {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}
