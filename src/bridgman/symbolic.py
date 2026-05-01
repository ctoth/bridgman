"""Dimensional analysis of sympy expression trees."""

from fractions import Fraction

from sympy import (
    Abs,
    Add,
    Max,
    Min,
    cos,
    cosh,
    Eq,
    exp,
    Float,
    Integer,
    log,
    Mul,
    Number,
    NumberSymbol,
    Pow,
    Rational,
    sin,
    sinh,
    Symbol,
    tan,
    tanh,
    atan2,
)
from sympy.core.relational import Relational

from bridgman.dimensions import Dimensions, _clean, dims_equal, is_dimensionless, mul_dims


class DimensionalError(Exception):
    """Raised when dimensions are inconsistent (e.g. adding m + v)."""


_DIMENSIONLESS_ARG_FUNCTIONS = {
    sin,
    cos,
    tan,
    exp,
    log,
    sinh,
    cosh,
    tanh,
}


def _dims_of_same_dimension_args(
    expr,
    dim_map: dict[str, Dimensions],
    context: str,
) -> Dimensions:
    dims_list = [dims_of_expr(arg, dim_map) for arg in expr.args]
    if not dims_list:
        return {}

    first = dims_list[0]
    for i, dims in enumerate(dims_list[1:], 1):
        if not dims_equal(first, dims):
            raise DimensionalError(
                f"Dimensional mismatch in {context}: "
                f"argument 0 has {first}, argument {i} has {dims}"
            )
    return first


def _pow_dims_frac(d: Dimensions, exp: Fraction) -> Dimensions:
    """Raise dimensions to a fractional power.

    Multiplies each exponent by exp. If any result is not an integer,
    raises DimensionalError.
    """
    result: dict[str, int] = {}
    for k, v in d.items():
        new_exp = Fraction(v) * exp
        if new_exp.denominator != 1:
            raise DimensionalError(
                f"Fractional dimension exponent: {k}^{new_exp} "
                f"(from {k}^{v} raised to power {exp})"
            )
        result[k] = int(new_exp)
    return _clean(result)


def _pow_exponent_fraction(exponent) -> Fraction:
    """Convert a SymPy power exponent to an exact-enough Fraction."""
    if isinstance(exponent, Rational):
        return Fraction(exponent.p, exponent.q)
    if isinstance(exponent, Float):
        raise DimensionalError(
            f"floating exponent in power expression is not dimensionally exact: {exponent}"
        )
    raise DimensionalError(f"non-numeric exponent in power expression: {exponent}")


def _dims_of_dimensionless_arg_function(expr, dim_map: dict[str, Dimensions]) -> Dimensions:
    for arg in expr.args:
        arg_dims = dims_of_expr(arg, dim_map)
        if not is_dimensionless(arg_dims):
            raise DimensionalError(
                f"{expr.func.__name__} argument must be dimensionless; got {arg_dims}"
            )
    return {}


def dims_of_expr(expr, dim_map: dict[str, Dimensions]) -> Dimensions:
    """Compute the dimensions of a sympy expression.

    Args:
        expr: A sympy expression.
        dim_map: Maps symbol names (strings) to their Dimensions dicts.

    Returns:
        The resulting Dimensions dict.

    Raises:
        KeyError: If a symbol is not found in dim_map.
        DimensionalError: If dimensions are inconsistent (e.g. in addition).
    """
    if isinstance(expr, Symbol):
        name = expr.name
        if name not in dim_map:
            raise KeyError(name)
        return _clean(dict(dim_map[name]))

    if isinstance(expr, (Number, NumberSymbol)):
        # Numeric constants (Integer, Rational, Float, pi, etc.) are dimensionless
        return {}

    if isinstance(expr, Mul):
        result: Dimensions = {}
        for arg in expr.args:
            result = mul_dims(result, dims_of_expr(arg, dim_map))
        return result

    if isinstance(expr, Pow):
        base_dims = dims_of_expr(expr.args[0], dim_map)
        exponent = expr.args[1]

        if is_dimensionless(base_dims):
            return {}

        # Convert exponent to Fraction for exact arithmetic.
        exp_frac = _pow_exponent_fraction(exponent)

        if exp_frac.denominator == 1:
            # Integer power — use simple multiplication
            return _clean({k: v * int(exp_frac) for k, v in base_dims.items()})
        else:
            return _pow_dims_frac(base_dims, exp_frac)

    if isinstance(expr, Add):
        return _dims_of_same_dimension_args(expr, dim_map, "addition")

    if isinstance(expr, Relational):
        raise DimensionalError("Nested relational expressions are not dimension terms")

    if getattr(expr, "func", None) in _DIMENSIONLESS_ARG_FUNCTIONS:
        return _dims_of_dimensionless_arg_function(expr, dim_map)

    if getattr(expr, "func", None) == atan2:
        _dims_of_same_dimension_args(expr, dim_map, "atan2")
        return {}

    if getattr(expr, "func", None) == Abs:
        return dims_of_expr(expr.args[0], dim_map)

    if getattr(expr, "func", None) in {Min, Max}:
        return _dims_of_same_dimension_args(expr, dim_map, expr.func.__name__)

    raise DimensionalError(f"Unsupported sympy expression type: {type(expr).__name__}")


def verify_expr(eq, dim_map: dict[str, Dimensions]) -> bool:
    """Verify that both sides of a sympy relation have the same dimensions.

    Args:
        eq: A sympy Eq or inequality expression.
        dim_map: Maps symbol names (strings) to their Dimensions dicts.

    Returns:
        True if both sides have matching dimensions, False otherwise.
    """
    if not isinstance(eq, Relational):
        raise TypeError(f"Expected sympy relational expression, got {type(eq).__name__}")

    lhs_dims = dims_of_expr(eq.args[0], dim_map)
    rhs_dims = dims_of_expr(eq.args[1], dim_map)
    return dims_equal(lhs_dims, rhs_dims)
