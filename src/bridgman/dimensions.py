"""Dimensional analysis arithmetic for SI quantities."""

from bridgman._core import (
    canonicalize_dims,
    dims_equal,
    dims_signature,
    div_dims,
    mul_dims,
    parse_dims_signature,
    pow_dims,
)

# Type alias for dimensions: maps SI base dimension symbols to integer exponents
# SI base dimensions: M (mass), L (length), T (time), I (current),
# Theta (temperature), N (amount), J (luminous intensity)
Dimensions = dict[str, int]

SUPERSCRIPT = str.maketrans("-0123456789", "⁻⁰¹²³⁴⁵⁶⁷⁸⁹")

# Canonical ordering for display
DIM_ORDER = ["M", "L", "T", "I", "Theta", "N", "J"]


def _clean(d: Dimensions) -> Dimensions:
    """Remove zero-exponent entries."""
    return {k: v for k, v in d.items() if v != 0}


def is_dimensionless(d: Dimensions) -> bool:
    """True if all exponents are zero or dict is empty."""
    return _clean(d) == {}


def format_dims(d: Dimensions) -> str:
    """Human-readable formatting like 'M L T⁻²'."""
    cleaned = _clean(d)
    if not cleaned:
        return "1"

    def _sort_key(item):
        k, _ = item
        try:
            return DIM_ORDER.index(k)
        except ValueError:
            return len(DIM_ORDER)

    parts = []
    for sym, exp in sorted(cleaned.items(), key=_sort_key):
        if exp == 1:
            parts.append(sym)
        else:
            parts.append(f"{sym}{str(exp).translate(SUPERSCRIPT)}")
    return " ".join(parts)
