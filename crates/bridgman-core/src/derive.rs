//! Which kind `left op right` is, from dimensions and grade. `Kind::product`
//! asks it for quantities, and `compile` asks it to judge declared rows.
use crate::registry::CompiledKind;
use crate::{AffineRole, DerivationError, Dimensions, Grade, ProductOp};

pub(crate) struct Derivation {
    pub(crate) dimensions: Dimensions,
    pub(crate) grade: Grade,
    pub(crate) resolved: Resolved,
}
pub(crate) enum Resolved {
    /// Exactly one non-point kind, the dimensionless kind's neutral rule, or
    /// the kind a rate over a duration names.
    Kind(usize),
    /// Two or more non-point kinds, in declaration order.
    Twins(Vec<usize>),
    None,
}

/// The non-point kinds with `dimensions` at `grade`, in declaration order.
pub(crate) fn candidates(
    kinds: &[CompiledKind],
    dimensions: &Dimensions,
    grade: Grade,
) -> Vec<usize> {
    (0..kinds.len())
        .filter(|&i| {
            kinds[i].role != AffineRole::Point
                && kinds[i].grade == grade
                && kinds[i].dimensions.as_ref() == Some(dimensions)
        })
        .collect()
}

pub(crate) fn derive(
    kinds: &[CompiledKind],
    dimensionless: Option<usize>,
    duration: Option<usize>,
    left: usize,
    op: ProductOp,
    right: usize,
) -> Result<Derivation, DerivationError<usize>> {
    for point in [left, right] {
        if kinds[point].role == AffineRole::Point {
            return Err(DerivationError::Point {
                left,
                op,
                right,
                point,
            });
        }
    }
    let dimensions_of = |kind: usize| {
        kinds[kind]
            .dimensions
            .as_ref()
            .ok_or(DerivationError::UnresolvedDimensions { kind })
    };
    let (l, r) = (dimensions_of(left)?, dimensions_of(right)?);
    let (left_grade, right_grade) = (kinds[left].grade, kinds[right].grade);
    let grade = left_grade
        .product(op, right_grade)
        .ok_or(DerivationError::Ungraded {
            left,
            op,
            right,
            left_grade,
            right_grade,
        })?;
    let dimensions = match op {
        ProductOp::Div => l / r,
        ProductOp::Mul | ProductOp::Dot | ProductOp::Wedge => l * r,
    };
    let neutral = dimensionless.and_then(|one| match op {
        ProductOp::Mul if right == one => Some(left),
        ProductOp::Mul if left == one => Some(right),
        ProductOp::Div if right == one => Some(left),
        ProductOp::Div if left == right => Some(one),
        ProductOp::Mul | ProductOp::Div | ProductOp::Dot | ProductOp::Wedge => None,
    });
    let mut candidates = candidates(kinds, &dimensions, grade);
    // A rate times a duration is what it is the rate of, and back.
    let rate = duration.and_then(|d| match op {
        ProductOp::Mul if right == d => kinds[left].rate_of,
        ProductOp::Mul if left == d => kinds[right].rate_of,
        ProductOp::Div if right == d => kinds[left].rate,
        ProductOp::Mul | ProductOp::Div | ProductOp::Dot | ProductOp::Wedge => None,
    });
    let resolved = match (neutral.or(rate), candidates.len()) {
        (Some(kind), _) => Resolved::Kind(kind),
        (None, 0) => Resolved::None,
        (None, 1) => Resolved::Kind(candidates.remove(0)),
        (None, _) => Resolved::Twins(candidates),
    };
    Ok(Derivation {
        dimensions,
        grade,
        resolved,
    })
}
