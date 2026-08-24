use bridgman_core::Dimensions;
use num_bigint::BigInt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

fn from_python(dimensions: &Bound<'_, PyDict>) -> PyResult<Dimensions> {
    dimensions
        .iter()
        .map(|(key, exponent)| Ok((key.extract::<String>()?, exponent.extract::<BigInt>()?)))
        .collect()
}

fn to_python(py: Python<'_>, dimensions: Dimensions) -> PyResult<Py<PyDict>> {
    let result = PyDict::new(py);
    for (key, exponent) in dimensions {
        result.set_item(key, exponent)?;
    }
    Ok(result.unbind())
}

#[pyfunction]
fn clean(py: Python<'_>, dimensions: &Bound<'_, PyDict>) -> PyResult<Py<PyDict>> {
    to_python(py, bridgman_core::clean(&from_python(dimensions)?))
}

#[pyfunction]
fn canonicalize_dims(py: Python<'_>, dimensions: &Bound<'_, PyDict>) -> PyResult<Py<PyDict>> {
    to_python(py, bridgman_core::canonicalize(&from_python(dimensions)?))
}

#[pyfunction]
fn mul_dims(
    py: Python<'_>,
    left: &Bound<'_, PyDict>,
    right: &Bound<'_, PyDict>,
) -> PyResult<Py<PyDict>> {
    to_python(
        py,
        bridgman_core::multiply(&from_python(left)?, &from_python(right)?),
    )
}

#[pyfunction]
fn div_dims(
    py: Python<'_>,
    left: &Bound<'_, PyDict>,
    right: &Bound<'_, PyDict>,
) -> PyResult<Py<PyDict>> {
    to_python(
        py,
        bridgman_core::divide(&from_python(left)?, &from_python(right)?),
    )
}

#[pyfunction]
fn pow_dims(
    py: Python<'_>,
    dimensions: &Bound<'_, PyDict>,
    exponent: BigInt,
) -> PyResult<Py<PyDict>> {
    to_python(
        py,
        bridgman_core::power(&from_python(dimensions)?, &exponent),
    )
}

#[pyfunction]
fn dims_equal(left: &Bound<'_, PyDict>, right: &Bound<'_, PyDict>) -> PyResult<bool> {
    Ok(bridgman_core::equal(
        &from_python(left)?,
        &from_python(right)?,
    ))
}

#[pyfunction]
fn is_dimensionless(dimensions: &Bound<'_, PyDict>) -> PyResult<bool> {
    Ok(bridgman_core::is_dimensionless(&from_python(dimensions)?))
}

#[pyfunction]
fn format_dims(dimensions: &Bound<'_, PyDict>) -> PyResult<String> {
    Ok(bridgman_core::format(&from_python(dimensions)?))
}

#[pyfunction]
fn dims_signature(dimensions: &Bound<'_, PyDict>) -> PyResult<String> {
    Ok(bridgman_core::signature(&from_python(dimensions)?))
}

#[pyfunction]
fn parse_dims_signature(py: Python<'_>, signature: &str) -> PyResult<Py<PyDict>> {
    let dimensions = bridgman_core::parse_signature(signature)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    to_python(py, dimensions)
}

#[pymodule]
fn _core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(clean, module)?)?;
    module.add_function(wrap_pyfunction!(canonicalize_dims, module)?)?;
    module.add_function(wrap_pyfunction!(mul_dims, module)?)?;
    module.add_function(wrap_pyfunction!(div_dims, module)?)?;
    module.add_function(wrap_pyfunction!(pow_dims, module)?)?;
    module.add_function(wrap_pyfunction!(dims_equal, module)?)?;
    module.add_function(wrap_pyfunction!(is_dimensionless, module)?)?;
    module.add_function(wrap_pyfunction!(format_dims, module)?)?;
    module.add_function(wrap_pyfunction!(dims_signature, module)?)?;
    module.add_function(wrap_pyfunction!(parse_dims_signature, module)?)?;
    Ok(())
}
