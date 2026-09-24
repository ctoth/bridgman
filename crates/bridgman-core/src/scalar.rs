use num_bigint::{BigInt, ParseBigIntError};
use num_rational::{BigRational, ParseRatioError};
use num_traits::{One, ToPrimitive, Zero};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;
use thiserror::Error;

use crate::QuantityError;

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
    /// A finite binary64 value, exactly; `None` for a nonfinite one.
    pub(crate) fn from_f64(value: f64) -> Option<Self> {
        Some(Self {
            rational: BigRational::from_float(value)?,
            pi_exponent: BigInt::zero(),
        })
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

impl fmt::Display for ExactScalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.encoded())
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

/// A finite sum of rational multiples of powers of pi. On the wire it is the
/// list of its terms.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExactValue {
    terms: BTreeMap<BigInt, BigRational>,
}
impl ExactValue {
    pub fn from_scalar(value: ExactScalar) -> Self {
        Self::from_terms([value])
    }
    pub(crate) fn from_terms(terms: impl IntoIterator<Item = ExactScalar>) -> Self {
        let mut result = Self::default();
        for term in terms {
            result.add_term(term)
        }
        result
    }
    pub fn terms(&self) -> impl Iterator<Item = ExactScalar> + '_ {
        self.terms
            .iter()
            .map(|(pi_exponent, rational)| ExactScalar {
                rational: rational.clone(),
                pi_exponent: pi_exponent.clone(),
            })
    }
    /// The value as one term, when it is exactly one nonzero term.
    pub fn monomial(&self) -> Option<ExactScalar> {
        let mut terms = self.terms();
        let term = terms.next()?;
        terms.next().is_none().then_some(term)
    }
    fn add_term(&mut self, value: ExactScalar) {
        let total = self
            .terms
            .get(&value.pi_exponent)
            .cloned()
            .unwrap_or_else(BigRational::zero)
            + value.rational;
        if total.is_zero() {
            self.terms.remove(&value.pi_exponent);
        } else {
            self.terms.insert(value.pi_exponent, total);
        }
    }
    pub fn add(&self, rhs: &Self) -> Self {
        let mut result = self.clone();
        for term in rhs.terms() {
            result.add_term(term)
        }
        result
    }
    pub fn sub(&self, rhs: &Self) -> Self {
        let mut result = self.clone();
        for mut term in rhs.terms() {
            term.rational = -term.rational;
            result.add_term(term)
        }
        result
    }
    pub fn multiply_scalar(&self, rhs: &ExactScalar) -> Self {
        let mut result = Self::default();
        for term in self.terms() {
            result.add_term(term.multiply(rhs));
        }
        result
    }
    pub fn divide_scalar(&self, rhs: &ExactScalar) -> Result<Self, QuantityError> {
        if rhs.rational.is_zero() {
            return Err(QuantityError::DivisionByZero);
        }
        let mut result = Self::default();
        for term in self.terms() {
            result.add_term(term.divide(rhs).ok_or(QuantityError::DivisionByZero)?);
        }
        Ok(result)
    }
    pub fn to_f64(&self) -> Result<f64, QuantityError> {
        let mut result = 0.0;
        for term in self.terms() {
            result += term.to_f64().ok_or(QuantityError::NumericalFailure)?;
        }
        if result.is_finite() {
            Ok(result)
        } else {
            Err(QuantityError::NumericalFailure)
        }
    }
}
impl Serialize for ExactValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.terms())
    }
}
impl<'de> Deserialize<'de> for ExactValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::from_terms(Vec::<ExactScalar>::deserialize(
            deserializer,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_exact_value_cannot_be_divided_by_zero() {
        assert_eq!(
            ExactValue::default().divide_scalar(&ExactScalar::zero()),
            Err(QuantityError::DivisionByZero)
        );
    }
    #[test]
    fn exact_value_wire_form_is_its_terms() {
        let value: ExactValue = serde_json::from_str(r#"["1/2","1/2","1*pi^1"]"#).unwrap();
        assert_eq!(serde_json::to_string(&value).unwrap(), r#"["1","1*pi^1"]"#);
        assert_eq!(value.monomial(), None);
        let three = ExactScalar::parse("3").unwrap();
        assert_eq!(
            ExactValue::from_scalar(three.clone()).monomial(),
            Some(three)
        );
    }
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
