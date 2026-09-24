use bridgman_core::{
    count_pi_groups_exact, pi_groups_exact, Catalog, CatalogError, Dimensions, Grade, KindDecl,
    OperationDecl, ProductOp, QuantityError, RateFault, Registry, CATALOG_SCHEMA,
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
        .cast::<PyDict>()
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
            let item = value.cast::<PyDict>()?;
            let dimensions = item
                .get_item("dimensions")?
                .ok_or_else(|| PyValueError::new_err("missing field dimensions"))?;
            declarations.push(KindDecl {
                id: item_string(item, "name")?,
                dimensions: Some(checked_dims(&dimensions)?),
                grade: Grade::Scalar,
                difference_kind: None,
                minimum: None,
                rate_of: None,
            });
        }
        let mut operations = Vec::new();
        for value in rules.iter() {
            let item = value.cast::<PyDict>()?;
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
            dimensionless: None,
            time: None,
            kinds: declarations,
            units: vec![],
            operations,
        })
        .map_err(catalog_error)?;
        Ok(Self { registry })
    }
    fn kind_dimensions<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyDict>> {
        let kind = self.registry.kind(name).map_err(quantity_error)?;
        dict(py, kind.dimensions().map_err(quantity_error)?)
    }
    fn result_kind(&self, left: &str, op: &str, right: &str) -> PyResult<String> {
        let kind = |id| self.registry.kind(id).map_err(quantity_error);
        let result = kind(left)?
            .product(product_op(op)?, kind(right)?)
            .map_err(quantity_error)?;
        Ok(result.id().into())
    }
    fn kinds_with_dimensions(&self, value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
        let target = checked_dims(value)?;
        let mut result = Vec::new();
        for kind in self.registry.kinds() {
            if kind.dimensions().map_err(quantity_error)? == &target {
                result.push(kind.id().into());
            }
        }
        Ok(result)
    }
}

/// Python boundary: each catalog fault becomes a tagged tuple carrying its fields.
fn catalog_error(error: CatalogError) -> PyErr {
    match error {
        CatalogError::Schema { expected, actual } => {
            PyValueError::new_err(("schema", expected, actual))
        }
        CatalogError::Json(source) => {
            PyValueError::new_err(("invalid_catalog", source.to_string()))
        }
        CatalogError::Yaml(source) => {
            PyValueError::new_err(("invalid_catalog", source.to_string()))
        }
        CatalogError::Duplicate { record, id } => {
            PyValueError::new_err(("duplicate", record.name(), id))
        }
        CatalogError::Unknown { record, id } => {
            PyValueError::new_err(("unknown", record.name(), id))
        }
        CatalogError::EmptyId { record } => PyValueError::new_err(("empty_id", record.name())),
        CatalogError::UnresolvedDimensions(id) => {
            PyValueError::new_err(("unresolved_dimensions", id))
        }
        CatalogError::AffineDimensionMismatch { point, difference } => {
            PyValueError::new_err(("affine_dimension_mismatch", point, difference))
        }
        CatalogError::NestedAffineSpace { point, difference } => {
            PyValueError::new_err(("nested_affine_space", point, difference))
        }
        CatalogError::ZeroScale { unit } => PyValueError::new_err(("zero_scale", unit)),
        CatalogError::NonFiniteConversion { unit } => {
            PyValueError::new_err(("nonfinite_conversion", unit))
        }
        CatalogError::IncompatibleReference {
            unit,
            reference,
            kind,
        } => PyValueError::new_err(("incompatible_reference", unit, reference, kind)),
        CatalogError::NonIdentityReference { unit, reference } => {
            PyValueError::new_err(("non_identity_reference", unit, reference))
        }
        CatalogError::InvalidMinimum { kind } => PyValueError::new_err(("invalid_minimum", kind)),
        CatalogError::InvalidDimensionlessKind(kind) => {
            PyValueError::new_err(("invalid_dimensionless_kind", kind))
        }
        CatalogError::ConflictingOperationRule { left, op, right } => {
            PyValueError::new_err(("duplicate_rule", left, op.to_string(), right))
        }
        CatalogError::InvalidOperationRule {
            left,
            op,
            right,
            result,
            dimensions,
            grade,
        } => PyValueError::new_err((
            "invalid_dimensions",
            left,
            op.to_string(),
            right,
            result,
            dimensions.signature(),
            u8::from(grade),
        )),
        CatalogError::DerivedOperationRule {
            left,
            op,
            right,
            result,
            derived,
        } => PyValueError::new_err(("derived_rule", left, op.to_string(), right, result, derived)),
        CatalogError::CommutativeQuotient { left, right } => {
            PyValueError::new_err(("commutative_quotient", left, right))
        }
        CatalogError::UngradedOperationRule {
            left,
            op,
            right,
            left_grade,
            right_grade,
        } => PyValueError::new_err((
            "ungraded_rule",
            left,
            op.to_string(),
            right,
            u8::from(left_grade),
            u8::from(right_grade),
        )),
        CatalogError::PointOperationRule {
            left,
            op,
            right,
            point,
        } => PyValueError::new_err(("point_rule", left, op.to_string(), right, point)),
        CatalogError::InvalidTimeKind(id) => PyValueError::new_err(("invalid_time_kind", id)),
        CatalogError::InvalidRate { rate, of, fault } => match fault {
            RateFault::NoTimeKind => {
                PyValueError::new_err(("invalid_rate", rate, of, "no_time_kind"))
            }
            RateFault::PointKind(kind) => {
                PyValueError::new_err(("invalid_rate", rate, of, "point_kind", kind))
            }
            RateFault::Mismatch { dimensions, grade } => PyValueError::new_err((
                "invalid_rate",
                rate,
                of,
                "mismatch",
                dimensions.signature(),
                u8::from(grade),
            )),
            RateFault::AlsoRateOf(kind) => {
                PyValueError::new_err(("invalid_rate", rate, of, "also_rate_of", kind))
            }
        },
        CatalogError::QudvDocument(source) => {
            PyValueError::new_err(("invalid_qudv_document", source.to_string()))
        }
        CatalogError::EmptySourceHash => PyValueError::new_err(("empty_source_hash",)),
        CatalogError::NonMonomialScale { unit } => {
            PyValueError::new_err(("non_monomial_scale", unit))
        }
        CatalogError::MixedApproximateSum { unit } => {
            PyValueError::new_err(("mixed_approximate_sum", unit))
        }
        CatalogError::ProvenanceEncoding(source) => {
            PyValueError::new_err(("provenance_encoding", source.to_string()))
        }
    }
}

/// Python boundary: each refused operation becomes a tagged tuple carrying its fields.
fn quantity_error(error: QuantityError) -> PyErr {
    match error {
        QuantityError::Unknown { record, id } => {
            PyValueError::new_err(("unknown", record.name(), id))
        }
        QuantityError::UnresolvedDimensions(id) => {
            PyValueError::new_err(("unresolved_dimensions", id))
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
        QuantityError::KindMismatch { expected, actual } => {
            PyValueError::new_err(("kind_mismatch", expected, actual))
        }
        QuantityError::UnsupportedOperation {
            operation,
            left,
            right,
        } => PyValueError::new_err(("unsupported_operation", operation.to_string(), left, right)),
        QuantityError::OffsetOnLinearKind { unit, kind } => {
            PyValueError::new_err(("offset_on_linear_kind", unit, kind))
        }
        QuantityError::BelowMinimum {
            kind,
            unit,
            minimum,
            value,
        } => PyValueError::new_err((
            "below_minimum",
            kind,
            unit,
            minimum.encoded(),
            value.encoded(),
        )),
        QuantityError::UnitKindMismatch { unit, kind } => {
            PyValueError::new_err(("unit_kind_mismatch", unit, kind))
        }
        QuantityError::AmbiguousKind(symbol) => PyValueError::new_err(("ambiguous_kind", symbol)),
        QuantityError::AmbiguousUnit(symbol) => PyValueError::new_err(("ambiguous_unit", symbol)),
        QuantityError::NoProductKind {
            left,
            op,
            right,
            dimensions,
            grade,
        } => PyValueError::new_err((
            "no_product_kind",
            left,
            op.to_string(),
            right,
            dimensions.signature(),
            u8::from(grade),
        )),
        QuantityError::UngradedProduct {
            left,
            op,
            right,
            left_grade,
            right_grade,
        } => PyValueError::new_err((
            "ungraded_product",
            left,
            op.to_string(),
            right,
            u8::from(left_grade),
            u8::from(right_grade),
        )),
        QuantityError::UnresolvedTwin {
            left,
            op,
            right,
            twins,
        } => PyValueError::new_err(("unresolved_twin", left, op.to_string(), right, twins)),
        QuantityError::NonFiniteInput => PyValueError::new_err(("nonfinite_input",)),
        QuantityError::NumericalFailure => PyValueError::new_err(("numerical_failure",)),
        QuantityError::DivisionByZero => PyValueError::new_err(("division_by_zero",)),
        QuantityError::DisconnectedConversion => {
            PyValueError::new_err(("disconnected_conversion",))
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
