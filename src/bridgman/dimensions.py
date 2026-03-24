"""Dimensional analysis arithmetic for SI quantities."""

# Type alias for dimensions: maps SI base dimension symbols to integer exponents
# SI base dimensions: M (mass), L (length), T (time), I (current),
# THETA (temperature), N (amount), J (luminous intensity)
Dimensions = dict[str, int]

SUPERSCRIPT = str.maketrans("-0123456789", "⁻⁰¹²³⁴⁵⁶⁷⁸⁹")

# Canonical ordering for display
DIM_ORDER = ["M", "L", "T", "I", "THETA", "N", "J"]


def _clean(d: Dimensions) -> Dimensions:
    """Remove zero-exponent entries."""
    return {k: v for k, v in d.items() if v != 0}


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
    if n == 0:
        return {}
    return _clean({k: v * n for k, v in d.items()})


def dims_equal(d1: Dimensions, d2: Dimensions) -> bool:
    """Check dimensional equality, treating missing keys as 0."""
    return _clean(d1) == _clean(d2)


def is_dimensionless(d: Dimensions) -> bool:
    """True if all exponents are zero or dict is empty."""
    return _clean(d) == {}


def verify_equation(
    lhs: Dimensions, rhs_terms: list[Dimensions], ops: list[str]
) -> bool:
    """Verify dimensional consistency of an equation.

    ops are "mul" or "div" applied left to right across rhs_terms.
    len(ops) must equal len(rhs_terms) - 1.
    """
    if not rhs_terms:
        return is_dimensionless(lhs)

    result = rhs_terms[0]
    for i, op in enumerate(ops):
        if op == "mul":
            result = mul_dims(result, rhs_terms[i + 1])
        elif op == "div":
            result = div_dims(result, rhs_terms[i + 1])
        else:
            raise ValueError(f"Unknown op: {op}")
    return dims_equal(lhs, result)


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
