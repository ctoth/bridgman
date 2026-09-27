//! `bridgman._core`: a thin boundary over `bridgman-core`. Python values are
//! read once into core types, the core decides everything, and every refusal
//! is raised as the exception class of its own Rust error variant.
use std::collections::HashMap;
use std::ffi::CString;
use std::fmt;

use bridgman_core::{
    count_pi_groups_exact, pi_groups_exact, CatalogError, DerivationError, DimensionError,
    Dimensions, Exponent, Kind, Op, OperationParseError, ProductOp, QuantityError, Registry, Term,
    CATALOG_SCHEMA, SI_BASES,
};
use num_bigint::BigInt;
use num_rational::BigRational;
use pyo3::exceptions::{PyException, PyRuntimeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBool, PyDict, PyInt, PyTuple, PyType};
use serde::Serialize;
use strum::VariantNames;

// ---------------------------------------------------------------------------
// Exceptions, one class per Rust error variant.

/// The root of every exception the core raises.
static BRIDGMAN_ERROR: PyOnceLock<Py<PyType>> = PyOnceLock::new();

fn bridgman_error(py: Python<'_>) -> PyResult<&Py<PyType>> {
    BRIDGMAN_ERROR.get_or_try_init(py, || {
        PyErr::new_type(
            py,
            c"bridgman.BridgmanError",
            Some(c"A refusal by the Rust core."),
            Some(&py.get_type::<PyException>()),
            None,
        )
    })
}

/// A class `bridgman.<qualname>` deriving from `base`.
fn new_class(py: Python<'_>, qualname: &str, base: &Bound<'_, PyType>) -> PyResult<Py<PyType>> {
    let name = CString::new(format!("bridgman.{qualname}"))
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    let class = PyErr::new_type(py, &name, None, Some(base), None)?;
    class.bind(py).setattr("__module__", "bridgman")?;
    class.bind(py).setattr("__qualname__", qualname)?;
    Ok(class)
}

/// A Rust error enum as Python sees it: a class named for the enum, and one
/// subclass per variant, named for the variant and set on the enum's class.
struct Family {
    base: Py<PyType>,
    variants: HashMap<&'static str, Py<PyType>>,
}

/// A Rust error enum raised into Python. Its variant names and fields come
/// from the enum's own derives; nothing here lists them.
trait Raise: Serialize + fmt::Display + VariantNames {
    const NAME: &'static str;
    fn family() -> &'static PyOnceLock<Family>;
    fn variant(&self) -> &'static str;
    /// The error this one wraps, raised as its own exception and set as the
    /// cause of this one.
    fn cause(&self, py: Python<'_>) -> Option<PyErr>;
}

fn family<E: Raise>(py: Python<'_>) -> PyResult<&'static Family> {
    E::family().get_or_try_init(py, || {
        let base = new_class(py, E::NAME, bridgman_error(py)?.bind(py))?;
        let mut variants = HashMap::new();
        for &variant in E::VARIANTS {
            let class = new_class(py, &format!("{}.{variant}", E::NAME), base.bind(py))?;
            base.bind(py).setattr(variant, class.clone_ref(py))?;
            variants.insert(variant, class);
        }
        base.bind(py)
            .setattr("variants", PyTuple::new(py, E::VARIANTS)?)?;
        Ok(Family { base, variants })
    })
}

/// `error` as an exception of its variant's class, carrying its message and,
/// as `fields`, what the variant names.
fn raised<E: Raise>(py: Python<'_>, error: &E) -> PyErr {
    exception(py, error).unwrap_or_else(|failure| failure)
}

fn exception<E: Raise>(py: Python<'_>, error: &E) -> PyResult<PyErr> {
    // Total: `family` made a class for every name in `E::VARIANTS`, and
    // `variant` is one of them (both come from the enum's derives).
    let class = &family::<E>(py)?.variants[error.variant()];
    let report = serde_json::to_string(error)
        .map_err(|failure| PyRuntimeError::new_err(failure.to_string()))?;
    let fields = py
        .import("json")?
        .call_method1("loads", (report,))?
        .call_method1("get", ("fields",))?;
    let value = class.bind(py).call1((error.to_string(),))?;
    value.setattr("fields", fields)?;
    let raised = PyErr::from_value(value);
    raised.set_cause(py, error.cause(py));
    Ok(raised)
}

macro_rules! leaf {
    ($error:ty) => {
        impl Raise for $error {
            const NAME: &'static str = stringify!($error);
            fn family() -> &'static PyOnceLock<Family> {
                static FAMILY: PyOnceLock<Family> = PyOnceLock::new();
                &FAMILY
            }
            fn variant(&self) -> &'static str {
                self.into()
            }
            fn cause(&self, _: Python<'_>) -> Option<PyErr> {
                None
            }
        }
    };
}
leaf!(DimensionError);
leaf!(OperationParseError);

impl<K: Serialize + fmt::Display> Raise for DerivationError<K> {
    const NAME: &'static str = "DerivationError";
    fn family() -> &'static PyOnceLock<Family> {
        static FAMILY: PyOnceLock<Family> = PyOnceLock::new();
        &FAMILY
    }
    fn variant(&self) -> &'static str {
        self.into()
    }
    fn cause(&self, _: Python<'_>) -> Option<PyErr> {
        None
    }
}
impl Raise for CatalogError {
    const NAME: &'static str = "CatalogError";
    fn family() -> &'static PyOnceLock<Family> {
        static FAMILY: PyOnceLock<Family> = PyOnceLock::new();
        &FAMILY
    }
    fn variant(&self) -> &'static str {
        self.into()
    }
    fn cause(&self, py: Python<'_>) -> Option<PyErr> {
        if let Self::Derivation(inner) = self {
            Some(raised(py, inner))
        } else {
            None
        }
    }
}
impl Raise for QuantityError<'_> {
    const NAME: &'static str = "QuantityError";
    fn family() -> &'static PyOnceLock<Family> {
        static FAMILY: PyOnceLock<Family> = PyOnceLock::new();
        &FAMILY
    }
    fn variant(&self) -> &'static str {
        self.into()
    }
    fn cause(&self, py: Python<'_>) -> Option<PyErr> {
        if let Self::Derivation(inner) = self {
            Some(raised(py, inner))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Dimensions: exponents are `int` or `fractions.Fraction` in Python and exact
// rationals in the core.

/// Python boundary: an `int` or a `Fraction` becomes an exact rational once.
fn rational(value: &Bound<'_, PyAny>) -> PyResult<BigRational> {
    let refused = || PyTypeError::new_err("an exponent must be an int or a Fraction");
    if value.is_instance_of::<PyBool>() {
        return Err(refused());
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(BigRational::from_integer(value.extract::<BigInt>()?));
    }
    let fraction = value.py().import("fractions")?.getattr("Fraction")?;
    if !value.is_instance(&fraction)? {
        return Err(refused());
    }
    Ok(BigRational::new(
        value.getattr("numerator")?.extract::<BigInt>()?,
        value.getattr("denominator")?.extract::<BigInt>()?,
    ))
}

/// An exact rational as Python writes it: an `int` when it is one.
fn python_rational<'py>(py: Python<'py>, value: &BigRational) -> PyResult<Bound<'py, PyAny>> {
    if value.is_integer() {
        return Ok(value.to_integer().into_pyobject(py)?.into_any());
    }
    py.import("fractions")?
        .getattr("Fraction")?
        .call1((value.numer().clone(), value.denom().clone()))
}

/// Python boundary: a dict of exponents becomes `Dimensions` once.
fn checked_dims(value: &Bound<'_, PyAny>) -> PyResult<Dimensions> {
    let dict = value
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("dimensions must be a dict"))?;
    let mut powers = Vec::with_capacity(dict.len());
    for (key, power) in dict.iter() {
        powers.push((key.extract::<String>()?, rational(&power)?));
    }
    Ok(Dimensions::from_rational_powers(powers))
}

/// `Dimensions` as a dict, in the core's signature order.
fn dict<'py>(py: Python<'py>, dimensions: &Dimensions) -> PyResult<Bound<'py, PyDict>> {
    let result = PyDict::new(py);
    for (id, power) in dimensions.powers() {
        result.set_item(id, python_rational(py, power)?)?;
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

/// Python boundary: an `int` or `Fraction` is an exact exponent, and `None`
/// one known only inexactly (a symbol or a float).
fn exponent(value: Option<&Bound<'_, PyAny>>) -> PyResult<Exponent> {
    value.map_or(Ok(Exponent::Inexact), |value| {
        rational(value).map(Exponent::Exact)
    })
}

/// `Dimensions::raised`.
#[pyfunction]
#[pyo3(signature = (value, power))]
fn pow_dims<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
    power: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let raised = checked_dims(value)?
        .raised(&exponent(power)?)
        .map_err(|error| raised(py, &error))?;
    dict(py, &raised)
}

/// `Dimensions::common`: the dimensions of terms that are added or compared.
#[pyfunction]
fn common_dims<'py>(
    py: Python<'py>,
    left: &Bound<'py, PyAny>,
    right: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let common = checked_dims(left)?
        .common(&checked_dims(right)?)
        .map_err(|error| raised(py, &error))?;
    dict(py, &common)
}

/// `Dimensions::transcendental`: the dimensions of exp, log, sin, ... of a
/// value with these dimensions.
#[pyfunction]
fn transcendental_dims<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let result = checked_dims(value)?
        .transcendental()
        .map_err(|error| raised(py, &error))?;
    dict(py, &result)
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
    let dimensions = Dimensions::parse_signature(signature).map_err(|error| raised(py, &error))?;
    dict(py, &dimensions)
}

fn extract_quantities(quantities: &Bound<'_, PyDict>) -> PyResult<Vec<(String, Dimensions)>> {
    let mut result = Vec::with_capacity(quantities.len());
    for (name, value) in quantities.iter() {
        result.push((name.extract::<String>()?, checked_dims(&value)?));
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
    Ok(PyTuple::new(py, groups)?.into_any().unbind())
}

// ---------------------------------------------------------------------------
// Kinds: a compiled catalog, asked by kind id.

#[pyclass]
struct NativeKindRegistry {
    registry: Registry,
}

impl NativeKindRegistry {
    fn kind(&self, py: Python<'_>, id: &str) -> PyResult<Kind<'_>> {
        self.registry.kind(id).map_err(|error| raised(py, &error))
    }
    /// Python boundary: a kind id is a value of that kind, and `None` a pure
    /// number, which has no kind.
    fn term(&self, py: Python<'_>, id: Option<&str>) -> PyResult<Term<'_>> {
        id.map_or(Ok(Term::Number), |id| self.kind(py, id).map(Term::Kind))
    }
}

fn operation<T>(py: Python<'_>, name: &str) -> PyResult<T>
where
    T: std::str::FromStr<Err = OperationParseError>,
{
    name.parse().map_err(|error| raised(py, &error))
}

/// A term as Python writes it: a kind id, or `None` for a pure number.
fn id(py: Python<'_>, term: Result<Term<'_>, QuantityError<'_>>) -> PyResult<Option<String>> {
    match term.map_err(|error| raised(py, &error))? {
        Term::Number => Ok(None),
        Term::Kind(kind) => Ok(Some(kind.id().to_owned())),
    }
}

#[pymethods]
impl NativeKindRegistry {
    /// A catalog document (JSON), read through the core's catalog schema.
    #[new]
    fn new(py: Python<'_>, catalog: &str) -> PyResult<Self> {
        Ok(Self {
            registry: Registry::from_json(catalog).map_err(|error| raised(py, &error))?,
        })
    }
    #[staticmethod]
    fn bundled() -> Self {
        Self {
            registry: bridgman_core::thermal().clone(),
        }
    }
    fn kinds(&self) -> Vec<String> {
        self.registry
            .kinds()
            .map(|kind| kind.id().to_owned())
            .collect()
    }
    fn kind_dimensions<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyDict>> {
        let kind = self.kind(py, name)?;
        dict(py, kind.dimensions().map_err(|error| raised(py, &error))?)
    }
    /// `Term::combine`: the kind of `left op right` for any operation, where
    /// `None` is a pure number.
    #[pyo3(signature = (left, op, right))]
    fn result_kind(
        &self,
        py: Python<'_>,
        left: Option<&str>,
        op: &str,
        right: Option<&str>,
    ) -> PyResult<Option<String>> {
        let op: Op = operation(py, op)?;
        id(py, self.term(py, left)?.combine(op, self.term(py, right)?))
    }
    /// `Term::power`, for an `int` or `Fraction` exponent, or `None` for an
    /// inexact one.
    #[pyo3(signature = (base, exponent))]
    fn power_kind(
        &self,
        py: Python<'_>,
        base: Option<&str>,
        exponent: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Option<String>> {
        let exponent = crate::exponent(exponent)?;
        id(py, self.term(py, base)?.power(&exponent))
    }
    /// `Term::absolute`: the kind of a value with its sign dropped.
    #[pyo3(signature = (kind))]
    fn absolute_kind(&self, py: Python<'_>, kind: Option<&str>) -> PyResult<Option<String>> {
        id(py, self.term(py, kind)?.absolute())
    }
    /// `Term::same`: the kind two compared values share.
    #[pyo3(signature = (left, right))]
    fn same_kind(
        &self,
        py: Python<'_>,
        left: Option<&str>,
        right: Option<&str>,
    ) -> PyResult<Option<String>> {
        id(py, self.term(py, left)?.same(self.term(py, right)?))
    }
    fn row_provenance(
        &self,
        py: Python<'_>,
        left: &str,
        op: &str,
        right: &str,
    ) -> PyResult<Option<String>> {
        let op: ProductOp = operation(py, op)?;
        Ok(self
            .kind(py, left)?
            .row_provenance(op, self.kind(py, right)?)
            .map_err(|error| raised(py, &error))?
            .map(str::to_owned))
    }
    fn kinds_with_dimensions(
        &self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<String>> {
        let target = checked_dims(value)?;
        let mut result = Vec::new();
        for kind in self.registry.kinds() {
            if kind.dimensions().map_err(|error| raised(py, &error))? == &target {
                result.push(kind.id().into());
            }
        }
        Ok(result)
    }
}

#[pymodule]
fn _core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    module.add_function(wrap_pyfunction!(canonicalize_dims, module)?)?;
    module.add_function(wrap_pyfunction!(mul_dims, module)?)?;
    module.add_function(wrap_pyfunction!(div_dims, module)?)?;
    module.add_function(wrap_pyfunction!(pow_dims, module)?)?;
    module.add_function(wrap_pyfunction!(common_dims, module)?)?;
    module.add_function(wrap_pyfunction!(transcendental_dims, module)?)?;
    module.add("SI_BASES", PyTuple::new(py, SI_BASES)?)?;
    module.add_function(wrap_pyfunction!(dims_equal, module)?)?;
    module.add_function(wrap_pyfunction!(dims_signature, module)?)?;
    module.add_function(wrap_pyfunction!(parse_dims_signature, module)?)?;
    module.add_function(wrap_pyfunction!(count_pi_groups, module)?)?;
    module.add_function(wrap_pyfunction!(pi_groups, module)?)?;
    module.add_class::<NativeKindRegistry>()?;
    module.add("CATALOG_SCHEMA", CATALOG_SCHEMA)?;
    module.add("BridgmanError", bridgman_error(py)?.clone_ref(py))?;
    module.add(
        "CatalogError",
        family::<CatalogError>(py)?.base.clone_ref(py),
    )?;
    module.add(
        "QuantityError",
        family::<QuantityError<'_>>(py)?.base.clone_ref(py),
    )?;
    module.add(
        "DerivationError",
        family::<DerivationError<String>>(py)?.base.clone_ref(py),
    )?;
    module.add(
        "DimensionError",
        family::<DimensionError>(py)?.base.clone_ref(py),
    )?;
    module.add(
        "OperationParseError",
        family::<OperationParseError>(py)?.base.clone_ref(py),
    )?;
    Ok(())
}
