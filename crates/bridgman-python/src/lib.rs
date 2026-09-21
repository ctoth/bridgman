use bridgman_core::{
    canonicalize_legacy_dims, count_pi_groups_exact, legacy_dims_equal, legacy_dims_signature,
    legacy_div_dims, legacy_mul_dims, legacy_pow_dims, pi_groups_exact, Catalog, Dimensions,
    KindDecl, LegacyDimensions, Op, OperationDecl, QuantityError, Registry, CATALOG_SCHEMA,
};
use num_bigint::BigInt;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::BTreeMap;

fn checked_dims(value: &Bound<'_, PyAny>) -> PyResult<LegacyDimensions> {
    let dict = value
        .downcast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("dimensions must be a dict"))?;
    let mut result = Vec::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(PyTypeError::new_err("dimension exponents must be int"));
        }
        result.push((key.extract::<String>()?, value.extract::<BigInt>()?));
    }
    Ok(result)
}

fn dict<'py>(
    py: Python<'py>,
    values: impl IntoIterator<Item = (String, BigInt)>,
) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(py);
    for (key, value) in values {
        if value != BigInt::from(0) {
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
    dict(py, canonicalize_legacy_dims(checked_dims(value)?))
}

#[pyfunction]
fn mul_dims<'py>(
    py: Python<'py>,
    left: &Bound<'py, PyAny>,
    right: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    dict(
        py,
        legacy_mul_dims(checked_dims(left)?, checked_dims(right)?),
    )
}

#[pyfunction]
fn div_dims<'py>(
    py: Python<'py>,
    left: &Bound<'py, PyAny>,
    right: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    dict(
        py,
        legacy_div_dims(checked_dims(left)?, checked_dims(right)?),
    )
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
        .extract::<BigInt>()
        .map_err(|_| PyTypeError::new_err("dimension exponent must be int"))?;
    dict(py, legacy_pow_dims(checked_dims(value)?, &power))
}

#[pyfunction]
fn dims_equal(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(legacy_dims_equal(checked_dims(left)?, checked_dims(right)?))
}

#[pyfunction]
fn dims_signature(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(legacy_dims_signature(checked_dims(value)?))
}

fn extract_quantities(quantities: &Bound<'_, PyDict>) -> PyResult<Vec<(String, LegacyDimensions)>> {
    let mut names = Vec::new();
    let mut dims = Vec::new();
    for (name, value) in quantities.iter() {
        let name = name.extract::<String>()?;
        if name.is_empty() {
            return Err(PyValueError::new_err(
                "quantity names must be non-empty strings",
            ));
        }
        names.push(name);
        dims.push(checked_dims(&value)?);
    }
    Ok(names.into_iter().zip(dims).collect())
}

#[pyfunction]
fn count_pi_groups(quantities: &Bound<'_, PyDict>) -> PyResult<usize> {
    Ok(count_pi_groups_exact(&extract_quantities(quantities)?))
}

#[pyfunction]
fn pi_groups(py: Python<'_>, quantities: &Bound<'_, PyDict>) -> PyResult<Py<PyAny>> {
    let mut groups = Vec::new();
    for values in pi_groups_exact(&extract_quantities(quantities)?) {
        let d = PyDict::new(py);
        for (name, value) in values {
            d.set_item(name, value)?;
        }
        groups.push(d);
    }
    Ok(pyo3::types::PyTuple::new(py, groups)?.into_any().unbind())
}

#[pyclass]
struct NativeKindRegistry {
    registry: Registry,
    kinds: Vec<(String, LegacyDimensions)>,
}

fn item_string(item: &Bound<'_, PyDict>, name: &str) -> PyResult<String> {
    item.get_item(name)?
        .ok_or_else(|| PyValueError::new_err(format!("missing field {name}")))?
        .extract()
}

#[pymethods]
impl NativeKindRegistry {
    #[new]
    fn new(kinds: &Bound<'_, PyList>, rules: &Bound<'_, PyList>) -> PyResult<Self> {
        let mut declarations = Vec::new();
        let mut ordered = Vec::new();
        for value in kinds.iter() {
            let item = value.downcast::<PyDict>()?;
            let name = item_string(item, "name")?;
            let dimensions_value = item
                .get_item("dimensions")?
                .ok_or_else(|| PyValueError::new_err("missing field dimensions"))?;
            let dimensions = checked_dims(&dimensions_value)?;
            declarations.push(KindDecl {
                id: name.clone(),
                dimensions: Some(core_dimensions(dimensions.clone())?),
                difference_kind: None,
            });
            ordered.push((name, dimensions));
        }
        let mut operations = Vec::new();
        for value in rules.iter() {
            let item = value.downcast::<PyDict>()?;
            let left = item_string(item, "left_kind")?;
            let op = item_string(item, "op")?;
            let right = item_string(item, "right_kind")?;
            let target = item_string(item, "result_kind")?;
            let op = match op.as_str() {
                "mul" => Op::Mul,
                "div" => Op::Div,
                _ => return Err(PyValueError::new_err(format!("invalid_operation:{op}"))),
            };
            let commutative: bool = item
                .get_item("commutative")?
                .map(|v| v.extract())
                .transpose()?
                .unwrap_or(false);
            operations.push(OperationDecl {
                left,
                op,
                right,
                result: target,
                commutative,
                provenance: None,
            });
        }
        let registry = Registry::compile(Catalog {
            schema: CATALOG_SCHEMA,
            provenance: BTreeMap::new(),
            kinds: declarations,
            units: vec![],
            operations,
        })
        .map_err(registry_error)?;
        Ok(Self {
            registry,
            kinds: ordered,
        })
    }
    fn kind_dimensions<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyDict>> {
        self.registry.kind(name).map_err(registry_error)?;
        let (_, dimensions) = self
            .kinds
            .iter()
            .find(|(id, _)| id == name)
            .expect("compiled kind");
        dict(py, dimensions.clone())
    }
    fn result_kind(&self, left: &str, op: &str, right: &str) -> PyResult<String> {
        let op = match op {
            "mul" => Op::Mul,
            "div" => Op::Div,
            _ => return Err(PyValueError::new_err(format!("invalid_operation:{op}"))),
        };
        let result = self
            .registry
            .result_kind(
                self.registry.kind(left).map_err(registry_error)?,
                op,
                self.registry.kind(right).map_err(registry_error)?,
            )
            .map_err(registry_error)?;
        Ok(self
            .registry
            .kind_id(result)
            .map_err(registry_error)?
            .into())
    }
    fn kinds_with_dimensions(&self, value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
        let target = core_dimensions(checked_dims(value)?)?;
        let mut result = Vec::new();
        for (name, _) in &self.kinds {
            let handle = self.registry.kind(name).map_err(registry_error)?;
            if self.registry.dimensions(handle).map_err(registry_error)? == &target {
                result.push(name.clone());
            }
        }
        Ok(result)
    }
}

fn core_dimensions(values: LegacyDimensions) -> PyResult<Dimensions> {
    Dimensions::from_rational_powers(
        canonicalize_legacy_dims(values)
            .into_iter()
            .map(|(id, value)| (id, (value, 1.into()))),
    )
    .map_err(|error| PyValueError::new_err(error.to_string()))
}
fn registry_error(error: QuantityError) -> PyErr {
    match error {
        QuantityError::Schema { expected, actual } => {
            PyValueError::new_err(("schema", expected, actual))
        }
        QuantityError::Duplicate { record, id } => PyValueError::new_err(("duplicate", record, id)),
        QuantityError::Unknown { record, id } => PyValueError::new_err(("unknown", record, id)),
        QuantityError::UnresolvedDimensions(id) => {
            PyValueError::new_err(("unresolved_dimensions", id))
        }
        QuantityError::RegistryMismatch => PyValueError::new_err(("registry_mismatch",)),
        QuantityError::KindMismatch { left, right } => {
            PyValueError::new_err(("kind_mismatch", left, right))
        }
        QuantityError::UnitKindMismatch { unit, kind } => {
            PyValueError::new_err(("unit_kind_mismatch", unit, kind))
        }
        QuantityError::AmbiguousKind(symbol) => PyValueError::new_err(("ambiguous_kind", symbol)),
        QuantityError::AmbiguousUnit(symbol) => PyValueError::new_err(("ambiguous_unit", symbol)),
        QuantityError::MissingOperationRule { left, op, right } => {
            PyValueError::new_err(("missing_rule", left, op, right))
        }
        QuantityError::ConflictingOperationRule { left, op, right } => {
            PyValueError::new_err(("duplicate_rule", left, op, right))
        }
        QuantityError::InvalidOperationRule => PyValueError::new_err(("invalid_dimensions",)),
        QuantityError::UnsupportedAffineOperation => {
            PyValueError::new_err(("unsupported_affine_operation",))
        }
        QuantityError::NonFiniteInput => PyValueError::new_err(("nonfinite_input",)),
        QuantityError::NumericalFailure => PyValueError::new_err(("numerical_failure",)),
        QuantityError::DivisionByZero => PyValueError::new_err(("division_by_zero",)),
        QuantityError::DisconnectedConversion => {
            PyValueError::new_err(("disconnected_conversion",))
        }
        QuantityError::InvalidCatalog(message) => {
            PyValueError::new_err(("invalid_catalog", message))
        }
    }
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
    module.add_class::<NativeKindRegistry>()?;
    Ok(())
}
