//! A kind's grade in the geometric algebra of three-dimensional space (G3).
//! The algebra is fixed; there is no signature field.
use crate::ProductOp;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(try_from = "u8", into = "u8")]
pub enum Grade {
    #[default]
    Scalar,
    Vector,
    Bivector,
    Trivector,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("grade {0} is outside G3's grades 0 to 3")]
pub struct GradeError(pub u8);

impl Grade {
    pub const ALL: [Self; 4] = [Self::Scalar, Self::Vector, Self::Bivector, Self::Trivector];
    pub const fn index(self) -> u8 {
        match self {
            Self::Scalar => 0,
            Self::Vector => 1,
            Self::Bivector => 2,
            Self::Trivector => 3,
        }
    }
    /// The grade of `self op other`, or `None` when G3 gives that product no
    /// single grade: two non-scalars under `mul`, a non-scalar divisor, a scalar
    /// under `dot` or `wedge` (whose scalar product is `mul`), or a wedge past 3.
    pub fn product(self, op: ProductOp, other: Self) -> Option<Self> {
        let (a, b) = (self.index(), other.index());
        let index = match op {
            ProductOp::Mul if a == 0 || b == 0 => a + b,
            ProductOp::Div if b == 0 => a,
            ProductOp::Dot if a != 0 && b != 0 => a.abs_diff(b),
            ProductOp::Wedge if a != 0 && b != 0 => a + b,
            ProductOp::Mul | ProductOp::Div | ProductOp::Dot | ProductOp::Wedge => return None,
        };
        Self::try_from(index).ok()
    }
}
impl TryFrom<u8> for Grade {
    type Error = GradeError;
    fn try_from(index: u8) -> Result<Self, GradeError> {
        Self::ALL
            .into_iter()
            .find(|g| g.index() == index)
            .ok_or(GradeError(index))
    }
}
impl From<Grade> for u8 {
    fn from(grade: Grade) -> u8 {
        grade.index()
    }
}
impl fmt::Display for Grade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.index().fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CatalogError, Registry};

    /// `Grade::product` for every operation and pair of grades, written out:
    /// row is the left grade, column the right, `None` where G3 gives none.
    const PRODUCTS: [(ProductOp, [[Option<u8>; 4]; 4]); 4] = [
        (
            ProductOp::Mul,
            [
                [Some(0), Some(1), Some(2), Some(3)],
                [Some(1), None, None, None],
                [Some(2), None, None, None],
                [Some(3), None, None, None],
            ],
        ),
        (
            ProductOp::Div,
            [
                [Some(0), None, None, None],
                [Some(1), None, None, None],
                [Some(2), None, None, None],
                [Some(3), None, None, None],
            ],
        ),
        (
            ProductOp::Dot,
            [
                [None, None, None, None],
                [None, Some(0), Some(1), Some(2)],
                [None, Some(1), Some(0), Some(1)],
                [None, Some(2), Some(1), Some(0)],
            ],
        ),
        (
            ProductOp::Wedge,
            [
                [None, None, None, None],
                [None, Some(2), Some(3), None],
                [None, Some(3), None, None],
                [None, None, None, None],
            ],
        ),
    ];

    #[test]
    fn grades_outside_g3_are_refused() {
        for (op, table) in PRODUCTS {
            for left in Grade::ALL {
                for right in Grade::ALL {
                    let expected = table[usize::from(left.index())][usize::from(right.index())];
                    assert_eq!(
                        left.product(op, right),
                        expected.map(|index| Grade::try_from(index).unwrap()),
                        "{left} {op} {right}"
                    );
                }
            }
        }
        assert_eq!(Grade::try_from(4), Err(GradeError(4)));
        for grade in Grade::ALL {
            assert_eq!(Grade::try_from(u8::from(grade)), Ok(grade));
        }
        let catalog = "schema: 4\nkinds:\n  - {id: x, dimensions: {L: 1}, grade: 4}\nunits: []\n";
        assert!(matches!(
            Registry::from_yaml(catalog),
            Err(CatalogError::Yaml(_))
        ));
    }
}
