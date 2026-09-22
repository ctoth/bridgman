//! Buckingham Pi groups over exact dimension exponents.

use std::collections::BTreeSet;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

use crate::Dimensions;

/// The number of independent dimensionless groups the named quantities form.
pub fn count_pi_groups_exact(quantities: &[(String, Dimensions)]) -> usize {
    let (_, pivots) = rref(dimension_matrix(quantities), quantities.len());
    quantities.len() - pivots.len()
}

/// A deterministic integer basis of the dimensionless groups: each group maps
/// quantity names to nonzero integer exponents, first exponent positive.
pub fn pi_groups_exact(quantities: &[(String, Dimensions)]) -> Vec<Vec<(String, BigInt)>> {
    let (matrix, pivots) = rref(dimension_matrix(quantities), quantities.len());
    (0..quantities.len())
        .filter(|column| !pivots.contains(column))
        .map(|free| {
            let mut vector = vec![BigRational::zero(); quantities.len()];
            vector[free] = BigRational::one();
            for (row, &pivot) in pivots.iter().enumerate() {
                vector[pivot] = -matrix[row][free].clone();
            }
            let denominator = vector
                .iter()
                .fold(BigInt::one(), |d, x| lcm(d, x.denom().clone()));
            let mut integers: Vec<BigInt> = vector
                .iter()
                .map(|x| (x * &denominator).to_integer())
                .collect();
            let divisor = integers
                .iter()
                .filter(|x| !x.is_zero())
                .fold(BigInt::zero(), |d, x| gcd(d, x.abs()));
            if !divisor.is_zero() {
                for value in &mut integers {
                    *value /= &divisor;
                }
            }
            if integers
                .iter()
                .find(|x| !x.is_zero())
                .is_some_and(|x| x.is_negative())
            {
                for value in &mut integers {
                    *value = -value.clone();
                }
            }
            quantities
                .iter()
                .map(|(name, _)| name.clone())
                .zip(integers)
                .filter(|(_, value)| !value.is_zero())
                .collect()
        })
        .collect()
}
fn dimension_matrix(quantities: &[(String, Dimensions)]) -> Vec<Vec<BigRational>> {
    let rows: BTreeSet<&str> = quantities
        .iter()
        .flat_map(|(_, dims)| dims.powers().map(|(id, _)| id))
        .collect();
    rows.into_iter()
        .map(|row| {
            quantities
                .iter()
                .map(|(_, dims)| {
                    dims.powers()
                        .find(|(id, _)| *id == row)
                        .map_or_else(BigRational::zero, |(_, power)| power.clone())
                })
                .collect()
        })
        .collect()
}
fn rref(mut matrix: Vec<Vec<BigRational>>, columns: usize) -> (Vec<Vec<BigRational>>, Vec<usize>) {
    let mut pivot_row = 0;
    let mut pivots = Vec::new();
    for column in 0..columns {
        let Some(pivot) = (pivot_row..matrix.len()).find(|&row| !matrix[row][column].is_zero())
        else {
            continue;
        };
        matrix.swap(pivot_row, pivot);
        let value = matrix[pivot_row][column].clone();
        for item in &mut matrix[pivot_row] {
            *item /= value.clone();
        }
        for row in 0..matrix.len() {
            if row == pivot_row {
                continue;
            }
            let factor = matrix[row][column].clone();
            if factor.is_zero() {
                continue;
            }
            for col in 0..columns {
                let value = matrix[pivot_row][col].clone() * &factor;
                matrix[row][col] -= value;
            }
        }
        pivots.push(column);
        pivot_row += 1;
        if pivot_row == matrix.len() {
            break;
        }
    }
    (matrix, pivots)
}
fn gcd(mut left: BigInt, mut right: BigInt) -> BigInt {
    while !right.is_zero() {
        let remainder = &left % &right;
        left = right;
        right = remainder;
    }
    left.abs()
}
fn lcm(left: BigInt, right: BigInt) -> BigInt {
    if left.is_zero() {
        right
    } else {
        (&left / gcd(left.clone(), right.clone())) * right
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pi_basis_is_exact_and_deterministic() {
        let length = Dimensions::from_integer_powers([("L", 1)]);
        let q = vec![("x".into(), length.clone()), ("y".into(), length)];
        assert_eq!(
            pi_groups_exact(&q),
            vec![vec![("x".into(), 1.into()), ("y".into(), (-1).into())]]
        );
        assert_eq!(count_pi_groups_exact(&q), 1);
    }
    #[test]
    fn arbitrary_precision_exponents_are_exact() {
        let huge = BigRational::from_integer(BigInt::one() << 200usize);
        let q = vec![
            (
                "x".into(),
                Dimensions::from_rational_powers([("L", huge.clone())]),
            ),
            (
                "y".into(),
                Dimensions::from_rational_powers([("L", huge * BigInt::from(2))]),
            ),
        ];
        assert_eq!(
            pi_groups_exact(&q),
            vec![vec![("x".into(), 2.into()), ("y".into(), (-1).into())]]
        );
    }
}
