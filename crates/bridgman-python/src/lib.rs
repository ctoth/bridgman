use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::{BTreeMap, BTreeSet};

fn checked_dims(value: &Bound<'_, PyAny>) -> PyResult<Vec<(String, i64)>> {
    let dict = value
        .downcast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("dimensions must be a dict"))?;
    let mut result = Vec::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(PyTypeError::new_err("dimension exponents must be int"));
        }
        result.push((key.extract::<String>()?, value.extract::<i64>()?));
    }
    Ok(result)
}

fn dict<'py>(
    py: Python<'py>,
    values: impl IntoIterator<Item = (String, i64)>,
) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(py);
    for (key, value) in values {
        if value != 0 {
            result.set_item(key, value)?;
        }
    }
    Ok(result)
}

#[pyfunction]
fn canonicalize_dims<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let mut result: Vec<(String, i64)> = Vec::new();
    for (key, power) in checked_dims(value)? {
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
    dict(py, result)
}

#[pyfunction]
fn mul_dims<'py>(
    py: Python<'py>,
    left: &Bound<'py, PyAny>,
    right: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let mut result = checked_dims(left)?;
    for (key, power) in checked_dims(right)? {
        if let Some((_, value)) = result.iter_mut().find(|(existing, _)| existing == &key) {
            *value += power;
        } else {
            result.push((key, power));
        }
    }
    dict(py, result)
}

#[pyfunction]
fn div_dims<'py>(
    py: Python<'py>,
    left: &Bound<'py, PyAny>,
    right: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let mut result = checked_dims(left)?;
    for (key, power) in checked_dims(right)? {
        if let Some((_, value)) = result.iter_mut().find(|(existing, _)| existing == &key) {
            *value -= power;
        } else {
            result.push((key, -power));
        }
    }
    dict(py, result)
}

#[pyfunction]
fn pow_dims<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
    power: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    if power.is_instance_of::<pyo3::types::PyBool>() {
        return Err(PyTypeError::new_err(
            "dimension exponent must be int, got bool",
        ));
    }
    let power = power
        .extract::<i64>()
        .map_err(|_| PyTypeError::new_err("dimension exponent must be int"))?;
    dict(
        py,
        checked_dims(value)?
            .into_iter()
            .map(|(key, value)| (key, value * power)),
    )
}

#[pyfunction]
fn dims_equal(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<bool> {
    let clean = |v: &Bound<'_, PyAny>| -> PyResult<BTreeMap<String, i64>> {
        Ok(checked_dims(v)?
            .into_iter()
            .filter(|(_, p)| *p != 0)
            .collect())
    };
    Ok(clean(left)? == clean(right)?)
}

#[pyfunction]
fn dims_signature(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let canonical = canonical_map(value)?;
    if canonical.is_empty() {
        return Ok("1".into());
    }
    let order = ["M", "L", "T", "I", "Theta", "N", "J"];
    let mut parts: Vec<_> = canonical.into_iter().collect();
    parts.sort_by_key(|(key, _)| {
        (
            order.iter().position(|x| x == key).unwrap_or(order.len()),
            key.clone(),
        )
    });
    Ok(parts
        .into_iter()
        .map(|(k, v)| format!("{k}:{v}"))
        .collect::<Vec<_>>()
        .join(","))
}

fn canonical_map(value: &Bound<'_, PyAny>) -> PyResult<BTreeMap<String, i64>> {
    let mut result = BTreeMap::new();
    for (key, power) in checked_dims(value)? {
        let key = if matches!(key.as_str(), "Θ" | "θ") {
            "Theta".into()
        } else {
            key
        };
        *result.entry(key).or_insert(0) += power;
    }
    result.retain(|_, p| *p != 0);
    Ok(result)
}

fn matrix(quantities: &Bound<'_, PyDict>) -> PyResult<(Vec<String>, Vec<Vec<BigRational>>)> {
    let mut names = Vec::new();
    let mut dims = Vec::new();
    let mut rows = BTreeSet::new();
    for (name, value) in quantities.iter() {
        let name = name.extract::<String>()?;
        if name.is_empty() {
            return Err(PyValueError::new_err(
                "quantity names must be non-empty strings",
            ));
        }
        let d = canonical_map(&value)?;
        rows.extend(d.keys().cloned());
        names.push(name);
        dims.push(d);
    }
    let matrix = rows
        .into_iter()
        .map(|row| {
            dims.iter()
                .map(|d| BigRational::from_integer((*d.get(&row).unwrap_or(&0)).into()))
                .collect()
        })
        .collect();
    Ok((names, matrix))
}

fn rref(mut a: Vec<Vec<BigRational>>, cols: usize) -> (Vec<Vec<BigRational>>, Vec<usize>) {
    let mut pivot_row = 0;
    let mut pivots = Vec::new();
    for col in 0..cols {
        let Some(pivot) = (pivot_row..a.len()).find(|&r| !a[r][col].is_zero()) else {
            continue;
        };
        a.swap(pivot_row, pivot);
        let p = a[pivot_row][col].clone();
        for v in &mut a[pivot_row] {
            *v /= p.clone();
        }
        for row in 0..a.len() {
            if row == pivot_row {
                continue;
            }
            let factor = a[row][col].clone();
            if factor.is_zero() {
                continue;
            }
            for c in 0..cols {
                let v = a[pivot_row][c].clone() * &factor;
                a[row][c] -= v;
            }
        }
        pivots.push(col);
        pivot_row += 1;
        if pivot_row == a.len() {
            break;
        }
    }
    (a, pivots)
}

#[pyfunction]
fn count_pi_groups(quantities: &Bound<'_, PyDict>) -> PyResult<usize> {
    let (names, m) = matrix(quantities)?;
    let (_, p) = rref(m, names.len());
    Ok(names.len() - p.len())
}

fn gcd(mut a: BigInt, mut b: BigInt) -> BigInt {
    while !b.is_zero() {
        let r = &a % &b;
        a = b;
        b = r;
    }
    a.abs()
}
fn lcm(a: BigInt, b: BigInt) -> BigInt {
    if a.is_zero() {
        b
    } else {
        (&a / gcd(a.clone(), b.clone())) * b
    }
}

#[pyfunction]
fn pi_groups(py: Python<'_>, quantities: &Bound<'_, PyDict>) -> PyResult<Py<PyAny>> {
    let (names, m) = matrix(quantities)?;
    let (a, pivots) = rref(m, names.len());
    let free = (0..names.len()).filter(|c| !pivots.contains(c));
    let mut groups = Vec::new();
    for free_col in free {
        let mut v = vec![BigRational::zero(); names.len()];
        v[free_col] = BigRational::one();
        for (row, &pivot) in pivots.iter().enumerate() {
            v[pivot] = -a[row][free_col].clone();
        }
        let denominator = v
            .iter()
            .fold(BigInt::one(), |d, x| lcm(d, x.denom().clone()));
        let mut ints: Vec<BigInt> = v.iter().map(|x| (x * &denominator).to_integer()).collect();
        let divisor = ints
            .iter()
            .filter(|x| !x.is_zero())
            .fold(BigInt::zero(), |d, x| gcd(d, x.abs()));
        if !divisor.is_zero() {
            for x in &mut ints {
                *x /= &divisor
            }
        }
        if ints
            .iter()
            .find(|x| !x.is_zero())
            .is_some_and(|x| x.is_negative())
        {
            for x in &mut ints {
                *x = -x.clone()
            }
        }
        let d = PyDict::new(py);
        for (name, value) in names.iter().zip(ints) {
            if !value.is_zero() {
                d.set_item(
                    name,
                    value.to_string().parse::<i64>().map_err(|_| {
                        PyValueError::new_err("pi exponent exceeds Python compatibility range")
                    })?,
                )?;
            }
        }
        groups.push(d);
    }
    Ok(pyo3::types::PyTuple::new(py, groups)?.into_any().unbind())
}

#[pymodule]
fn _core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(canonicalize_dims, module)?)?;
    module.add_function(wrap_pyfunction!(mul_dims, module)?)?;
    module.add_function(wrap_pyfunction!(div_dims, module)?)?;
    module.add_function(wrap_pyfunction!(pow_dims, module)?)?;
    module.add_function(wrap_pyfunction!(dims_equal, module)?)?;
    module.add_function(wrap_pyfunction!(dims_signature, module)?)?;
    module.add_function(wrap_pyfunction!(count_pi_groups, module)?)?;
    module.add_function(wrap_pyfunction!(pi_groups, module)?)?;
    Ok(())
}
