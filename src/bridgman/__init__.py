"""Bridgman: dimensional analysis arithmetic for SI quantities."""

from bridgman.dimensions import (
    Dimensions,
    mul_dims,
    div_dims,
    pow_dims,
    dims_equal,
    is_dimensionless,
    verify_equation,
    format_dims,
)

__all__ = [
    "Dimensions",
    "mul_dims",
    "div_dims",
    "pow_dims",
    "dims_equal",
    "is_dimensionless",
    "verify_equation",
    "format_dims",
]

try:
    from bridgman.symbolic import dims_of_expr, verify_expr, DimensionalError

    __all__ += ["dims_of_expr", "verify_expr", "DimensionalError"]
except ImportError:
    pass  # sympy not available
