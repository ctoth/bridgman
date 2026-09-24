//! A kind's grade in the geometric algebra of three-dimensional space (G3).
//! The algebra is fixed; there is no signature field.
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

    #[test]
    fn grades_outside_g3_are_refused() {
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
