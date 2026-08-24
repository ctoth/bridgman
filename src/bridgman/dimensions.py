"""Compatibility facade for the Rust dimension arithmetic core."""

from bridgman import _core

# Public compatibility alias: dimension maps remain ordinary Python dicts.
Dimensions = dict[str, int]

# Kept in Python because the Pi implementation imports the canonical ordering.
DIM_ORDER = ["M", "L", "T", "I", "Theta", "N", "J"]
SUPERSCRIPT = str.maketrans("-0123456789", "⁻⁰¹²³⁴⁵⁶⁷⁸⁹")


def _clean(d: Dimensions) -> Dimensions:
    """Remove zero-exponent entries."""
    return _core.clean(d)


def canonicalize_dims(d: Dimensions) -> Dimensions:
    """Normalize dimension keys and combine duplicate canonical keys."""
    return _core.canonicalize_dims(d)


def mul_dims(d1: Dimensions, d2: Dimensions) -> Dimensions:
    """Multiply quantities: add exponents."""
    return _core.mul_dims(d1, d2)


def div_dims(d1: Dimensions, d2: Dimensions) -> Dimensions:
    """Divide quantities: subtract exponents."""
    return _core.div_dims(d1, d2)


def pow_dims(d: Dimensions, n: int) -> Dimensions:
    """Raise to integer power: multiply all exponents by n."""
    if not isinstance(n, int) or isinstance(n, bool):
        raise TypeError(f"dimension exponent must be int, got {type(n).__name__}")
    return _core.pow_dims(d, n)


def dims_equal(d1: Dimensions, d2: Dimensions) -> bool:
    """Check dimensional equality, treating missing keys as 0."""
    return _core.dims_equal(d1, d2)


def is_dimensionless(d: Dimensions) -> bool:
    """True if all exponents are zero or dict is empty."""
    return _core.is_dimensionless(d)


def format_dims(d: Dimensions) -> str:
    """Human-readable formatting like 'M L T⁻²'."""
    for exponent in d.values():
        if exponent != 0 and exponent != 1:
            str(exponent)
    return _core.format_dims(d)


def dims_signature(d: Dimensions) -> str:
    """Return a canonical, zero-stripped dimension signature."""
    canonical = _core.canonicalize_dims(d)
    for exponent in canonical.values():
        str(exponent)
    return _core.dims_signature(canonical)


def parse_dims_signature(signature: str) -> Dimensions:
    """Parse a dimension signature produced by dims_signature."""
    if signature == "1":
        return _core.parse_dims_signature(signature)

    parsed: Dimensions = {}
    for part in signature.split(","):
        key, value = part.split(":", 1)
        parsed[key] = int(value)

    normalized = ",".join(f"{key}:{value}" for key, value in parsed.items())
    return _core.parse_dims_signature(normalized)
