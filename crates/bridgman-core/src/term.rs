//! A term of an expression over a registry's kinds: a value of a kind, or a
//! pure number, which has no kind. Whatever walks an expression (the Python
//! symbolic layer, for one) asks these operations and states no rule itself.
use crate::{Exponent, Kind, Op, Operation, QuantityError};
use num_bigint::BigInt;
use num_rational::BigRational;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Term<'r> {
    /// A pure number: it scales a kind, and is no term of a sum with one.
    Number,
    Kind(Kind<'r>),
}

impl<'r> Term<'r> {
    /// `self op other`. Two kinds combine by `Kind::combine`, and two numbers
    /// give a number. A number times a kind, or a kind divided by a number, is
    /// the kind scaled (`Kind::scaled`); a number divided by a kind is the
    /// kind's reciprocal. A number is no term of a sum, difference, dot or
    /// wedge with a kind (`NumberTerm`).
    pub fn combine(self, op: Op, other: Self) -> Result<Self, QuantityError<'r>> {
        let number_term = |kind| QuantityError::NumberTerm {
            operation: Operation::Binary(op),
            kind,
        };
        match (self, other) {
            (Self::Number, Self::Number) => Ok(Self::Number),
            (Self::Kind(left), Self::Kind(right)) => left.combine(op, right).map(Self::Kind),
            (Self::Kind(kind), Self::Number) => match op {
                Op::Mul => kind.scaled(Operation::Scale).map(Self::Kind),
                Op::Div => kind.scaled(Operation::DivideScalar).map(Self::Kind),
                Op::Add | Op::Sub | Op::Dot | Op::Wedge => Err(number_term(kind)),
            },
            (Self::Number, Self::Kind(kind)) => match op {
                Op::Mul => kind.scaled(Operation::Scale).map(Self::Kind),
                Op::Div => kind
                    .power(&Exponent::Exact(BigRational::from_integer(BigInt::from(
                        -1,
                    ))))
                    .map(Self::Kind),
                Op::Add | Op::Sub | Op::Dot | Op::Wedge => Err(number_term(kind)),
            },
        }
    }
    /// `self` raised to `exponent`: a number to any power is a number, and a
    /// kind's power is `Kind::power`.
    pub fn power(self, exponent: &Exponent) -> Result<Self, QuantityError<'r>> {
        match self {
            Self::Number => Ok(Self::Number),
            Self::Kind(kind) => kind.power(exponent).map(Self::Kind),
        }
    }
    /// `|self|`: a number stays a number, and a kind is `Kind::scaled`.
    pub fn absolute(self) -> Result<Self, QuantityError<'r>> {
        match self {
            Self::Number => Ok(Self::Number),
            Self::Kind(kind) => kind.scaled(Operation::Abs).map(Self::Kind),
        }
    }
    /// The term two compared values share: two numbers, or one kind
    /// (`Kind::same`). A number is not compared with a quantity.
    pub fn same(self, other: Self) -> Result<Self, QuantityError<'r>> {
        match (self, other) {
            (Self::Number, Self::Number) => Ok(Self::Number),
            (Self::Kind(left), Self::Kind(right)) => left.same(right).map(Self::Kind),
            (Self::Kind(kind), Self::Number) | (Self::Number, Self::Kind(kind)) => {
                Err(QuantityError::NumberTerm {
                    operation: Operation::Compare,
                    kind,
                })
            }
        }
    }
}
