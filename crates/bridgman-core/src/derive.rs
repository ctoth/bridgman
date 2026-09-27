//! Which kind `left op right` is, from dimensions and grade. `derive` states
//! the rule on dimensions and grade alone; `resolve` applies it to a catalog's
//! kinds and picks the kind. `Kind::product` asks `resolve` for quantities, and
//! `compile` asks it to judge declared rows.
use crate::registry::CompiledKind;
use crate::{AffineRole, DerivationError, Dimensions, Grade, ProductOp};

/// Dimensions at a grade in G3: what a product reads of a factor, and what
/// it gives, before any kind is chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graded {
    pub dimensions: Dimensions,
    pub grade: Grade,
}
/// The dimensions' signature and the grade: `M:1,L:2,T:-2 grade 0`.
impl std::fmt::Display for Graded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} grade {}", self.dimensions, self.grade)
    }
}

/// A kind as a factor of a product: its dimensions and grade, and its affine
/// role. `Kind::operand` gives a kind's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Operand {
    pub graded: Graded,
    pub role: AffineRole,
}

/// Which operand of `derive` a refusal names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}
impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Left => "left operand",
            Self::Right => "right operand",
        })
    }
}

/// A factor of a product: a kind, read as its `Operand`, or the result of
/// another product. A derived result is never a point and has no affine
/// role, so products chain (`(a*b)*c`) without one being invented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Factor<'a> {
    Kind(&'a Operand),
    Derived(&'a Graded),
}
impl<'a> Factor<'a> {
    /// The dimensions and grade the product reads.
    fn graded(self) -> &'a Graded {
        match self {
            Self::Kind(operand) => &operand.graded,
            Self::Derived(graded) => graded,
        }
    }
    fn is_point(self) -> bool {
        match self {
            Self::Kind(operand) => operand.role == AffineRole::Point,
            Self::Derived(_) => false,
        }
    }
}

/// `left op right` on dimensions and grade: a point kind takes part in no
/// product, and G3 must give the product a single grade. The refusal names
/// factors by `Side`; `DerivationError::map_kinds` names them otherwise.
pub fn derive(
    left: Factor<'_>,
    op: ProductOp,
    right: Factor<'_>,
) -> Result<Graded, DerivationError<Side>> {
    for (point, factor) in [(Side::Left, left), (Side::Right, right)] {
        if factor.is_point() {
            return Err(DerivationError::Point {
                left: Side::Left,
                op,
                right: Side::Right,
                point,
            });
        }
    }
    let (left, right) = (left.graded(), right.graded());
    let (left_grade, right_grade) = (left.grade, right.grade);
    let grade = left_grade
        .product(op, right_grade)
        .ok_or(DerivationError::Ungraded {
            left: Side::Left,
            op,
            right: Side::Right,
            left_grade,
            right_grade,
        })?;
    let dimensions = match op {
        ProductOp::Div => &left.dimensions / &right.dimensions,
        ProductOp::Mul | ProductOp::Dot | ProductOp::Wedge => &left.dimensions * &right.dimensions,
    };
    Ok(Graded { dimensions, grade })
}

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

/// A catalog kind as an operand of `derive`.
pub(crate) fn operand(
    kinds: &[CompiledKind],
    kind: usize,
) -> Result<Operand, DerivationError<usize>> {
    let compiled = &kinds[kind];
    Ok(Operand {
        graded: Graded {
            dimensions: compiled
                .dimensions
                .clone()
                .ok_or(DerivationError::UnresolvedDimensions { kind })?,
            grade: compiled.grade,
        },
        role: compiled.role,
    })
}

/// `derive` on two catalog kinds, and the kind it names: the dimensionless
/// kind's neutral rule, a rate over a duration, or the kinds with the product's
/// dimensions and grade.
pub(crate) fn resolve(
    kinds: &[CompiledKind],
    dimensionless: Option<usize>,
    duration: Option<usize>,
    left: usize,
    op: ProductOp,
    right: usize,
) -> Result<Derivation, DerivationError<usize>> {
    let Graded { dimensions, grade } = derive(
        Factor::Kind(&operand(kinds, left)?),
        op,
        Factor::Kind(&operand(kinds, right)?),
    )
    .map_err(|error| {
        error.map_kinds(|side| match side {
            Side::Left => left,
            Side::Right => right,
        })
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn declared(powers: &[(&str, i64)], grade: Grade, role: AffineRole) -> Operand {
        Operand {
            graded: Graded {
                dimensions: Dimensions::from_integer_powers(powers.iter().copied()),
                grade,
            },
            role,
        }
    }
    use Factor::Kind;

    #[test]
    fn a_derived_factor_chains_a_product() {
        let mass = declared(&[("M", 1)], Grade::Scalar, AffineRole::Linear);
        let velocity = declared(&[("L", 1), ("T", -1)], Grade::Vector, AffineRole::Linear);
        let duration = declared(&[("T", 1)], Grade::Scalar, AffineRole::Difference);
        // (mass * velocity) * duration, through the derived momentum.
        let momentum = derive(Kind(&mass), ProductOp::Mul, Kind(&velocity)).unwrap();
        let chained = derive(Factor::Derived(&momentum), ProductOp::Mul, Kind(&duration));
        assert_eq!(
            chained,
            Ok(Graded {
                dimensions: Dimensions::from_integer_powers([("M", 1), ("L", 1)]),
                grade: Grade::Vector,
            })
        );
        // The same as the direct dimension product.
        let direct = &(&Dimensions::from_integer_powers([("M", 1)])
            * &Dimensions::from_integer_powers([("L", 1), ("T", -1)]))
            * &Dimensions::from_integer_powers([("T", 1)]);
        assert_eq!(chained.unwrap().dimensions, direct);
        // A derived factor on the right, too: duration * (mass * velocity).
        let momentum = derive(Kind(&mass), ProductOp::Mul, Kind(&velocity)).unwrap();
        assert_eq!(
            derive(Kind(&duration), ProductOp::Mul, Factor::Derived(&momentum))
                .map(|graded| graded.dimensions),
            Ok(direct)
        );
    }
    #[test]
    fn a_graded_displays_its_dimensions_and_grade() {
        let energy = Graded {
            dimensions: Dimensions::from_integer_powers([("M", 1), ("L", 2), ("T", -2)]),
            grade: Grade::Scalar,
        };
        let velocity = Graded {
            dimensions: Dimensions::from_integer_powers([("L", 1), ("T", -1)]),
            grade: Grade::Vector,
        };
        assert_eq!(energy.to_string(), "M:1,L:2,T:-2 grade 0");
        assert_eq!(velocity.to_string(), "L:1,T:-1 grade 1");
    }
    #[test]
    fn a_point_kind_factor_is_refused_on_either_side() {
        let instant = declared(&[("T", 1)], Grade::Scalar, AffineRole::Point);
        let mass = Graded {
            dimensions: Dimensions::from_integer_powers([("M", 1)]),
            grade: Grade::Scalar,
        };
        let derived = Factor::Derived(&mass);
        for (left, right, point) in [
            (derived, Kind(&instant), Side::Right),
            (Kind(&instant), derived, Side::Left),
        ] {
            assert_eq!(
                derive(left, ProductOp::Mul, right),
                Err(DerivationError::Point {
                    left: Side::Left,
                    op: ProductOp::Mul,
                    right: Side::Right,
                    point,
                })
            );
        }
    }

    #[test]
    fn dimensions_and_grades_derive_without_kinds() {
        let force = declared(
            &[("M", 1), ("L", 1), ("T", -2)],
            Grade::Vector,
            AffineRole::Linear,
        );
        let displacement = declared(&[("L", 1)], Grade::Vector, AffineRole::Linear);
        let duration = declared(&[("T", 1)], Grade::Scalar, AffineRole::Difference);
        let (force, displacement, duration) = (Kind(&force), Kind(&displacement), Kind(&duration));
        assert_eq!(
            derive(force, ProductOp::Wedge, displacement),
            Ok(Graded {
                dimensions: Dimensions::from_integer_powers([("M", 1), ("L", 2), ("T", -2)]),
                grade: Grade::Bivector,
            })
        );
        assert_eq!(
            derive(force, ProductOp::Div, duration),
            Ok(Graded {
                dimensions: Dimensions::from_integer_powers([("M", 1), ("L", 1), ("T", -3)]),
                grade: Grade::Vector,
            })
        );
        assert_eq!(
            derive(force, ProductOp::Mul, displacement),
            Err(DerivationError::Ungraded {
                left: Side::Left,
                op: ProductOp::Mul,
                right: Side::Right,
                left_grade: Grade::Vector,
                right_grade: Grade::Vector,
            })
        );
    }
    #[test]
    fn a_point_takes_part_in_no_product() {
        let instant = declared(&[("T", 1)], Grade::Scalar, AffineRole::Point);
        let power = declared(
            &[("M", 1), ("L", 2), ("T", -3)],
            Grade::Scalar,
            AffineRole::Linear,
        );
        assert_eq!(
            derive(Kind(&power), ProductOp::Mul, Kind(&instant)),
            Err(DerivationError::Point {
                left: Side::Left,
                op: ProductOp::Mul,
                right: Side::Right,
                point: Side::Right,
            })
        );
    }
}
