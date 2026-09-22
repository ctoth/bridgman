use num_bigint::{BigInt, ParseBigIntError};
use num_rational::{BigRational, ParseRatioError};
use num_traits::{One, ToPrimitive, Zero};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactScalar {
    pub rational: BigRational,
    pub pi_exponent: BigInt,
}

#[derive(Clone, Debug, PartialEq, Error)]
pub enum ScalarError {
    #[error("exact scalar {text:?} has an invalid rational coefficient")]
    Coefficient {
        text: String,
        #[source]
        source: ParseRatioError,
    },
    #[error("exact scalar {text:?} has an invalid pi exponent")]
    PiExponent {
        text: String,
        #[source]
        source: ParseBigIntError,
    },
}

impl ExactScalar {
    pub fn one() -> Self {
        Self {
            rational: BigRational::one(),
            pi_exponent: BigInt::zero(),
        }
    }
    pub fn zero() -> Self {
        Self {
            rational: BigRational::zero(),
            pi_exponent: BigInt::zero(),
        }
    }
    pub fn parse(value: &str) -> Result<Self, ScalarError> {
        let (coefficient, pi_exponent) = match value.split_once("*pi^") {
            Some((c, p)) => (
                c,
                p.parse().map_err(|source| ScalarError::PiExponent {
                    text: value.into(),
                    source,
                })?,
            ),
            None => (value, BigInt::zero()),
        };
        Ok(Self {
            rational: coefficient
                .parse()
                .map_err(|source| ScalarError::Coefficient {
                    text: value.into(),
                    source,
                })?,
            pi_exponent,
        })
    }
    pub fn multiply(&self, rhs: &Self) -> Self {
        Self {
            rational: &self.rational * &rhs.rational,
            pi_exponent: &self.pi_exponent + &rhs.pi_exponent,
        }
    }
    pub fn divide(&self, rhs: &Self) -> Option<Self> {
        if rhs.rational.is_zero() {
            None
        } else {
            Some(Self {
                rational: &self.rational / &rhs.rational,
                pi_exponent: &self.pi_exponent - &rhs.pi_exponent,
            })
        }
    }
    pub fn to_f64(&self) -> Option<f64> {
        let coefficient = self.rational.to_f64()?;
        let value = coefficient * std::f64::consts::PI.powi(self.pi_exponent.to_i32()?);
        value.is_finite().then_some(value)
    }
    pub fn encoded(&self) -> String {
        let base = if self.rational.denom().is_one() {
            self.rational.numer().to_string()
        } else {
            format!("{}/{}", self.rational.numer(), self.rational.denom())
        };
        if self.pi_exponent.is_zero() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_pi_arithmetic_does_not_wrap_machine_integers() {
        let a = ExactScalar::parse("1*pi^2147483647").unwrap();
        let b = a.multiply(&ExactScalar::parse("1*pi^1").unwrap());
        assert_eq!(b.encoded(), "1*pi^2147483648");
        assert_eq!(b.to_f64(), None);
        assert_eq!(b.divide(&a).unwrap().encoded(), "1*pi^1");
    }
    #[test]
    fn invalid_scalars_name_the_failing_part() {
        assert!(matches!(
            ExactScalar::parse("1/0"),
            Err(ScalarError::Coefficient { .. })
        ));
        assert!(matches!(
            ExactScalar::parse("1*pi^x"),
            Err(ScalarError::PiExponent { .. })
        ));
    }
}
