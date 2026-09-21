use std::collections::{BTreeMap, BTreeSet};

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

pub type LegacyDimensions = Vec<(String, BigInt)>;

pub fn canonicalize_legacy_dims(values: LegacyDimensions) -> LegacyDimensions {
    let mut result: LegacyDimensions = Vec::new();
    for (key, power) in values {
        let key = match key.as_str() {
            "Θ" | "θ" => "Theta".into(),
            _ => key,
        };
        if let Some((_, value)) = result.iter_mut().find(|(existing, _)| existing == &key) {
            *value += power;
        } else {
            result.push((key, power));
        }
    }
    result.retain(|(_, power)| !power.is_zero());
    result
}

pub fn legacy_mul_dims(mut left: LegacyDimensions, right: LegacyDimensions) -> LegacyDimensions {
    combine(&mut left, right, false);
    left
}
pub fn legacy_div_dims(mut left: LegacyDimensions, right: LegacyDimensions) -> LegacyDimensions {
    combine(&mut left, right, true);
    left
}
fn combine(left: &mut LegacyDimensions, right: LegacyDimensions, subtract: bool) {
    for (key, mut power) in right {
        if subtract {
            power = -power;
        }
        if let Some((_, value)) = left.iter_mut().find(|(existing, _)| existing == &key) {
            *value += power;
        } else {
            left.push((key, power));
        }
    }
    left.retain(|(_, power)| !power.is_zero());
}
pub fn legacy_pow_dims(values: LegacyDimensions, power: &BigInt) -> LegacyDimensions {
    values
        .into_iter()
        .filter_map(|(key, value)| {
            let value = value * power;
            (!value.is_zero()).then_some((key, value))
        })
        .collect()
}
pub fn legacy_dims_equal(left: LegacyDimensions, right: LegacyDimensions) -> bool {
    clean_map(left) == clean_map(right)
}
fn clean_map(values: LegacyDimensions) -> BTreeMap<String, BigInt> {
    canonicalize_legacy_dims(values).into_iter().collect()
}
pub fn legacy_dims_signature(values: LegacyDimensions) -> String {
    let values = canonicalize_legacy_dims(values);
    if values.is_empty() {
        return "1".into();
    }
    let order = ["M", "L", "T", "I", "Theta", "N", "J"];
    let mut values = values;
    values.sort_by_key(|(key, _)| {
        (
            order.iter().position(|x| x == key).unwrap_or(order.len()),
            key.clone(),
        )
    });
    values
        .into_iter()
        .map(|(key, value)| format!("{key}:{value}"))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn count_pi_groups_exact(quantities: &[(String, LegacyDimensions)]) -> usize {
    let matrix = dimension_matrix(quantities);
    let (_, pivots) = rref(matrix, quantities.len());
    quantities.len() - pivots.len()
}
pub fn pi_groups_exact(quantities: &[(String, LegacyDimensions)]) -> Vec<LegacyDimensions> {
    let matrix = dimension_matrix(quantities);
    let (matrix, pivots) = rref(matrix, quantities.len());
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
fn dimension_matrix(quantities: &[(String, LegacyDimensions)]) -> Vec<Vec<BigRational>> {
    let maps: Vec<BTreeMap<_, _>> = quantities
        .iter()
        .map(|(_, dims)| canonicalize_legacy_dims(dims.clone()).into_iter().collect())
        .collect();
    let rows: BTreeSet<_> = maps.iter().flat_map(|dims| dims.keys().cloned()).collect();
    rows.into_iter()
        .map(|row| {
            maps.iter()
                .map(|dims| {
                    BigRational::from_integer(dims.get(&row).cloned().unwrap_or_else(BigInt::zero))
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
    fn arithmetic_is_arbitrary_precision() {
        let huge = BigInt::one() << 200usize;
        assert_eq!(
            legacy_pow_dims(vec![("L".into(), huge.clone())], &BigInt::from(2))[0].1,
            huge * 2
        );
    }
    #[test]
    fn pi_basis_is_exact_and_deterministic() {
        let q = vec![
            ("x".into(), vec![("L".into(), 1.into())]),
            ("y".into(), vec![("L".into(), 1.into())]),
        ];
        assert_eq!(
            pi_groups_exact(&q),
            vec![vec![("x".into(), 1.into()), ("y".into(), (-1).into())]]
        );
    }
}
