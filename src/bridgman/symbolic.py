"""Dimensional analysis of sympy expression trees."""

from fractions import Fraction

from sympy import (
    Add,
    Eq,
    Float,
    Integer,
    Mul,
    Number,
    NumberSymbol,
    Pow,
    Rational,
    Symbol,
)

from bridgman.dimensions import Dimensions, _clean, dims_equal, mul_dims


class DimensionalError(Exception):
    """Raised when dimensions are inconsistent (e.g. adding m + v)."""


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

        # Convert exponent to Fraction for exact arithmetic
        exp_frac = Fraction(exponent.p, exponent.q) if isinstance(exponent, Rational) else Fraction(float(exponent))

        if exp_frac.denominator == 1:
            # Integer power — use simple multiplication
            return _clean({k: v * int(exp_frac) for k, v in base_dims.items()})
        else:
            return _pow_dims_frac(base_dims, exp_frac)

    if isinstance(expr, Add):
        dims_list = [dims_of_expr(arg, dim_map) for arg in expr.args]
        first = dims_list[0]
        for i, d in enumerate(dims_list[1:], 1):
            if not dims_equal(first, d):
                raise DimensionalError(
                    f"Dimensional mismatch in addition: "
                    f"term 0 has {first}, term {i} has {d}"
                )
        return first

    if isinstance(expr, Eq):
        # For Eq, return dims of lhs (caller should use verify_expr instead)
        return dims_of_expr(expr.args[0], dim_map)

    raise TypeError(f"Unsupported sympy expression type: {type(expr).__name__}")


def verify_expr(eq, dim_map: dict[str, Dimensions]) -> bool:
    """Verify that both sides of a sympy Eq have the same dimensions.

    Args:
        eq: A sympy Eq expression.
        dim_map: Maps symbol names (strings) to their Dimensions dicts.

    Returns:
        True if both sides have matching dimensions, False otherwise.
    """
    if not isinstance(eq, Eq):
        raise TypeError(f"Expected sympy Eq, got {type(eq).__name__}")

    lhs_dims = dims_of_expr(eq.args[0], dim_map)
    rhs_dims = dims_of_expr(eq.args[1], dim_map)
    return dims_equal(lhs_dims, rhs_dims)
