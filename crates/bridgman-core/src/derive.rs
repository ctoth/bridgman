//! Which kind `left op right` is, from dimensions and grade. `Kind::product`
//! asks it for quantities, and `compile` asks it to judge declared rows.
use crate::registry::CompiledKind;
use crate::{AffineRole, Dimensions, Grade, ProductOp};

pub(crate) struct Derivation {
    pub(crate) dimensions: Dimensions,
    pub(crate) grade: Grade,
    pub(crate) resolved: Resolved,
}
pub(crate) enum Resolved {
    /// Exactly one non-point kind, or the dimensionless kind's neutral rule.
    Kind(usize),
    /// Two or more non-point kinds, in declaration order.
    Twins(Vec<usize>),
    None,
}
pub(crate) enum Underived {
    Point(usize),
    UnresolvedDimensions(usize),
    Ungraded { left: Grade, right: Grade },
}

pub(crate) fn derive(
    kinds: &[CompiledKind],
    dimensionless: Option<usize>,
    left: usize,
    op: ProductOp,
    right: usize,
) -> Result<Derivation, Underived> {
    for index in [left, right] {
        if kinds[index].role == AffineRole::Point {
            return Err(Underived::Point(index));
        }
    }
    let dimensions_of = |index: usize| {
        kinds[index]
            .dimensions
            .as_ref()
            .ok_or(Underived::UnresolvedDimensions(index))
    };
    let (l, r) = (dimensions_of(left)?, dimensions_of(right)?);
    let (lg, rg) = (kinds[left].grade, kinds[right].grade);
    let grade = lg.product(op, rg).ok_or(Underived::Ungraded {
        left: lg,
        right: rg,
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
    let mut candidates: Vec<usize> = (0..kinds.len())
        .filter(|&i| {
            kinds[i].role != AffineRole::Point
                && kinds[i].grade == grade
                && kinds[i].dimensions.as_ref() == Some(&dimensions)
        })
        .collect();
    let resolved = match (neutral, candidates.len()) {
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
