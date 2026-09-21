"""Dimensional analysis arithmetic for SI quantities."""

from bridgman._core import (
    canonicalize_dims,
    dims_equal,
    dims_signature,
    div_dims,
    mul_dims,
    pow_dims,
)

# Type alias for dimensions: maps SI base dimension symbols to integer exponents
# SI base dimensions: M (mass), L (length), T (time), I (current),
# Theta (temperature), N (amount), J (luminous intensity)
Dimensions = dict[str, int]

SUPERSCRIPT = str.maketrans("-0123456789", "⁻⁰¹²³⁴⁵⁶⁷⁸⁹")

# Canonical ordering for display
DIM_ORDER = ["M", "L", "T", "I", "Theta", "N", "J"]
_DIM_KEY_NORMALIZE = {
    chr(0x0398): "Theta",
    chr(0x03B8): "Theta",
    "Theta": "Theta",
}


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


def _signature_sort_key(item: tuple[str, int]) -> tuple[int, str]:
    key, _ = item
    try:
        return (DIM_ORDER.index(key), key)
    except ValueError:
        return (len(DIM_ORDER), key)


def parse_dims_signature(signature: str) -> Dimensions:
    """Parse a dimension signature produced by dims_signature."""
    if signature == "1":
        return {}
    result: Dimensions = {}
    for part in signature.split(","):
        key, value = part.split(":", 1)
        result[key] = int(value)
    return canonicalize_dims(result)
