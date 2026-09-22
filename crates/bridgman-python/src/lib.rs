use bridgman_core::{
    count_pi_groups_exact, pi_groups_exact, Catalog, Dimensions, KindDecl, OperationDecl,
    ProductOp, QuantityError, Registry, CATALOG_SCHEMA,
};
use num_bigint::BigInt;
use num_rational::BigRational;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::collections::BTreeMap;

/// Python boundary: a dict of integer exponents becomes `Dimensions` once.
fn checked_dims(value: &Bound<'_, PyAny>) -> PyResult<Dimensions> {
    let dict = value
        .downcast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("dimensions must be a dict"))?;
    let mut powers = Vec::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        if value.is_instance_of::<pyo3::types::PyBool>() {
            return Err(PyTypeError::new_err("dimension exponents must be int"));
        }
        powers.push((
            key.extract::<String>()?,
            BigRational::from_integer(value.extract::<BigInt>()?),
        ));
    }
    Ok(Dimensions::from_rational_powers(powers))
}

/// Python boundary: the Python API promises integer exponents.
fn dict<'py>(py: Python<'py>, dimensions: &Dimensions) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(py);
    for (id, power) in dimensions.powers() {
        if !power.is_integer() {
            return Err(PyValueError::new_err((
                "non_integer_exponent",
                id.to_owned(),
                power.to_string(),
            )));
        }
        result.set_item(id, power.to_integer())?;
    }
    Ok(result)
}

#[pyfunction]
fn canonicalize_dims<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    dict(py, &checked_dims(value)?)
}

#[pyfunction]
fn mul_dims<'py>(
    py: Python<'py>,
    left: &Bound<'py, PyAny>,
    right: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    dict(py, &(&checked_dims(left)? * &checked_dims(right)?))
}

#[pyfunction]
fn div_dims<'py>(
    py: Python<'py>,
    left: &Bound<'py, PyAny>,
    right: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    dict(py, &(&checked_dims(left)? / &checked_dims(right)?))
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
    dict(
        py,
        &checked_dims(value)?.pow(&BigRational::from_integer(power)),
    )
}

#[pyfunction]
fn dims_equal(left: &Bound<'_, PyAny>, right: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(checked_dims(left)? == checked_dims(right)?)
}

#[pyfunction]
fn dims_signature(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(checked_dims(value)?.signature())
}

#[pyfunction]
fn parse_dims_signature<'py>(py: Python<'py>, signature: &str) -> PyResult<Bound<'py, PyDict>> {
    let dimensions = Dimensions::parse_signature(signature)
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
    dict(py, &dimensions)
}

fn extract_quantities(quantities: &Bound<'_, PyDict>) -> PyResult<Vec<(String, Dimensions)>> {
    let mut result = Vec::with_capacity(quantities.len());
    for (name, value) in quantities.iter() {
        let name = name.extract::<String>()?;
        if name.is_empty() {
            return Err(PyValueError::new_err(
                "quantity names must be non-empty strings",
            ));
        }
        result.push((name, checked_dims(&value)?));
    }
    Ok(result)
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
}

fn item_string(item: &Bound<'_, PyDict>, name: &str) -> PyResult<String> {
    item.get_item(name)?
        .ok_or_else(|| PyValueError::new_err(format!("missing field {name}")))?
        .extract()
}

/// Python boundary: an operation name is parsed once, by the core's parser.
fn product_op(name: &str) -> PyResult<ProductOp> {
    name.parse()
        .map_err(|_| PyValueError::new_err(("invalid_operation", name.to_owned())))
}

#[pymethods]
impl NativeKindRegistry {
    #[new]
    fn new(kinds: &Bound<'_, PyList>, rules: &Bound<'_, PyList>) -> PyResult<Self> {
        let mut declarations = Vec::new();
        for value in kinds.iter() {
            let item = value.downcast::<PyDict>()?;
            let dimensions = item
                .get_item("dimensions")?
                .ok_or_else(|| PyValueError::new_err("missing field dimensions"))?;
            declarations.push(KindDecl {
                id: item_string(item, "name")?,
                dimensions: Some(checked_dims(&dimensions)?),
                difference_kind: None,
            });
        }
        let mut operations = Vec::new();
        for value in rules.iter() {
            let item = value.downcast::<PyDict>()?;
            let commutative: bool = item
                .get_item("commutative")?
                .map(|v| v.extract())
                .transpose()?
                .unwrap_or(false);
            operations.push(OperationDecl {
                left: item_string(item, "left_kind")?,
                op: product_op(&item_string(item, "op")?)?,
                right: item_string(item, "right_kind")?,
                result: item_string(item, "result_kind")?,
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
        Ok(Self { registry })
    }
    fn kind_dimensions<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyDict>> {
        let kind = self.registry.kind(name).map_err(registry_error)?;
        dict(py, self.registry.dimensions(kind).map_err(registry_error)?)
    }
    fn result_kind(&self, left: &str, op: &str, right: &str) -> PyResult<String> {
        let result = self
            .registry
            .result_kind(
                self.registry.kind(left).map_err(registry_error)?,
                product_op(op)?,
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
        let target = checked_dims(value)?;
        let mut result = Vec::new();
        for kind in self.registry.kinds() {
            if self.registry.dimensions(kind).map_err(registry_error)? == &target {
                result.push(self.registry.kind_id(kind).map_err(registry_error)?.into());
            }
        }
        Ok(result)
    }
}

/// Python boundary: each error becomes a tagged tuple carrying its fields.
fn registry_error(error: QuantityError) -> PyErr {
    match error {
        QuantityError::Schema { expected, actual } => {
            PyValueError::new_err(("schema", expected, actual))
        }
        QuantityError::CatalogJson(source) => {
            PyValueError::new_err(("invalid_catalog", source.to_string()))
        }
        QuantityError::Duplicate { record, id } => {
            PyValueError::new_err(("duplicate", record.name(), id))
        }
        QuantityError::Unknown { record, id } => {
            PyValueError::new_err(("unknown", record.name(), id))
        }
        QuantityError::EmptyId { record } => PyValueError::new_err(("empty_id", record.name())),
        QuantityError::UnresolvedDimensions(id) => {
            PyValueError::new_err(("unresolved_dimensions", id))
        }
        QuantityError::AffineDimensionMismatch { point, difference } => {
            PyValueError::new_err(("affine_dimension_mismatch", point, difference))
        }
        QuantityError::NestedAffineSpace { point, difference } => {
            PyValueError::new_err(("nested_affine_space", point, difference))
        }
        QuantityError::ZeroScale { unit } => PyValueError::new_err(("zero_scale", unit)),
        QuantityError::NonFiniteConversion { unit } => {
            PyValueError::new_err(("nonfinite_conversion", unit))
        }
        QuantityError::IncompatibleReference {
            unit,
            reference,
            kind,
        } => PyValueError::new_err(("incompatible_reference", unit, reference, kind)),
        QuantityError::NonIdentityReference { unit, reference } => {
            PyValueError::new_err(("non_identity_reference", unit, reference))
        }
        QuantityError::UnresolvedConversion { unit } => {
            PyValueError::new_err(("unresolved_conversion", unit))
        }
        QuantityError::ApproximateConversion { unit } => {
            PyValueError::new_err(("approximate_conversion", unit))
        }
        QuantityError::MissingCoherentScale { unit } => {
            PyValueError::new_err(("missing_coherent_scale", unit))
        }
        QuantityError::NoCanonicalUnit { kind } => {
            PyValueError::new_err(("no_canonical_unit", kind))
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
            PyValueError::new_err(("missing_rule", left, op.to_string(), right))
        }
        QuantityError::ConflictingOperationRule { left, op, right } => {
            PyValueError::new_err(("duplicate_rule", left, op.to_string(), right))
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
        QuantityError::QudvDocument(source) => {
            PyValueError::new_err(("invalid_qudv_document", source.to_string()))
        }
        QuantityError::EmptySourceHash => PyValueError::new_err(("empty_source_hash",)),
        QuantityError::NonMonomialScale { unit } => {
            PyValueError::new_err(("non_monomial_scale", unit))
        }
        QuantityError::MixedApproximateSum { unit } => {
            PyValueError::new_err(("mixed_approximate_sum", unit))
        }
        QuantityError::ProvenanceEncoding(source) => {
            PyValueError::new_err(("provenance_encoding", source.to_string()))
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
    module.add_function(wrap_pyfunction!(parse_dims_signature, module)?)?;
    module.add_function(wrap_pyfunction!(count_pi_groups, module)?)?;
    module.add_function(wrap_pyfunction!(pi_groups, module)?)?;
    module.add_class::<NativeKindRegistry>()?;
    Ok(())
}
