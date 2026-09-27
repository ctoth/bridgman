"""Dimensional analysis arithmetic, computed by the Rust core."""

from fractions import Fraction

from bridgman._core import (
    SI_BASES,
    canonicalize_dims,
    common_dims,
    dims_equal,
    dims_signature,
    div_dims,
    mul_dims,
    parse_dims_signature,
    pow_dims,
    transcendental_dims,
)

# Dimensions map base-dimension identifiers to exact exponents. The bases are
# open: the SI bases (SI_BASES, from the core) and any other identifier. An
# exponent is an int, or a Fraction when it is not whole (a root).
Dimensions = dict[str, int | Fraction]

SUPERSCRIPT = str.maketrans("-0123456789/", "⁻⁰¹²³⁴⁵⁶⁷⁸⁹ᐟ")


def is_dimensionless(d: Dimensions) -> bool:
    """True if all exponents are zero or dict is empty."""
    return canonicalize_dims(d) == {}


def format_dims(d: Dimensions) -> str:
    """Human-readable formatting like 'M L T⁻²', in the core's signature order."""
    canonical = canonicalize_dims(d)
    if not canonical:
        return "1"
    return " ".join(
        sym if exp == 1 else f"{sym}{str(exp).translate(SUPERSCRIPT)}"
        for sym, exp in canonical.items()
    )


__all__ = [
    "SI_BASES",
    "Dimensions",
    "canonicalize_dims",
    "common_dims",
    "dims_equal",
    "dims_signature",
    "div_dims",
    "format_dims",
    "is_dimensionless",
    "mul_dims",
    "parse_dims_signature",
    "pow_dims",
    "transcendental_dims",
]
