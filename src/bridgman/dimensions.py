"""Dimensional analysis arithmetic for SI quantities."""

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


def canonicalize_dims(d: Dimensions) -> Dimensions:
    """Normalize dimension keys and combine duplicate canonical keys."""
    result: Dimensions = {}
    for key, value in d.items():
        canonical_key = _DIM_KEY_NORMALIZE.get(key, key)
        result[canonical_key] = result.get(canonical_key, 0) + value
    return _clean(result)


def mul_dims(d1: Dimensions, d2: Dimensions) -> Dimensions:
    """Multiply quantities: add exponents."""
    result = dict(d1)
    for k, v in d2.items():
        result[k] = result.get(k, 0) + v
    return _clean(result)


def div_dims(d1: Dimensions, d2: Dimensions) -> Dimensions:
    """Divide quantities: subtract exponents."""
    result = dict(d1)
    for k, v in d2.items():
        result[k] = result.get(k, 0) - v
    return _clean(result)


def pow_dims(d: Dimensions, n: int) -> Dimensions:
    """Raise to integer power: multiply all exponents by n."""
    if not isinstance(n, int) or isinstance(n, bool):
        raise TypeError(f"dimension exponent must be int, got {type(n).__name__}")
    if n == 0:
        return {}
    return _clean({k: v * n for k, v in d.items()})


def dims_equal(d1: Dimensions, d2: Dimensions) -> bool:
    """Check dimensional equality, treating missing keys as 0."""
    return _clean(d1) == _clean(d2)


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


def dims_signature(d: Dimensions) -> str:
    """Return a canonical, zero-stripped dimension signature."""
    cleaned = canonicalize_dims(d)
    if not cleaned:
        return "1"
    return ",".join(
        f"{key}:{value}"
        for key, value in sorted(cleaned.items(), key=_signature_sort_key)
    )


def parse_dims_signature(signature: str) -> Dimensions:
    """Parse a dimension signature produced by dims_signature."""
    if signature == "1":
        return {}
    result: Dimensions = {}
    for part in signature.split(","):
        key, value = part.split(":", 1)
        result[key] = int(value)
    return canonicalize_dims(result)
