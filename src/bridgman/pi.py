"""Buckingham Pi helpers for dimensionless power products."""

from __future__ import annotations

from collections.abc import Mapping
from fractions import Fraction
from math import gcd
from functools import reduce

from bridgman.dimensions import DIM_ORDER, Dimensions, canonicalize_dims, is_dimensionless


class PiError(ValueError):
    """Raised when a Buckingham Pi input cannot be interpreted."""


def is_dimensionless_product(
    quantities: Mapping[str, Dimensions],
    exponents: Mapping[str, int],
) -> bool:
    """Return whether a named integer power product is dimensionless."""
    checked_quantities = _validate_quantities(quantities)
    checked_exponents = _validate_exponents(exponents)

    unknown = set(checked_exponents) - set(checked_quantities)
    if unknown:
        names = ", ".join(sorted(unknown))
        raise PiError(f"unknown quantity in product: {names}")

    total: Dimensions = {}
    for name, dims in checked_quantities.items():
        exponent = checked_exponents.get(name, 0)
        for key, value in dims.items():
            total[key] = total.get(key, 0) + value * exponent
    return is_dimensionless(canonicalize_dims(total))


def count_pi_groups(quantities: Mapping[str, Dimensions]) -> int:
    """Return the Buckingham count n - rank(A) for the quantity dimensions."""
    checked_quantities = _validate_quantities(quantities)
    matrix = _dimension_matrix(checked_quantities)
    return len(checked_quantities) - _rank(matrix)


def pi_groups(quantities: Mapping[str, Dimensions]) -> tuple[dict[str, int], ...]:
    """Return a deterministic integer basis for dimensionless monomials."""
    checked_quantities = _validate_quantities(quantities)
    names = tuple(checked_quantities)
    matrix = _dimension_matrix(checked_quantities)
    basis = _integer_nullspace_basis(matrix, len(names))

    groups: list[dict[str, int]] = []
    for vector in basis:
        group = {name: exponent for name, exponent in zip(names, vector) if exponent != 0}
        if group:
            groups.append(group)
    return tuple(groups)


def _validate_quantities(quantities: Mapping[str, Dimensions]) -> dict[str, Dimensions]:
    checked: dict[str, Dimensions] = {}
    for name, dims in quantities.items():
        if not isinstance(name, str) or name == "":
            raise PiError("quantity names must be non-empty strings")
        checked[name] = _validate_dims(name, dims)
    return checked


def _validate_dims(name: str, dims: Mapping[str, int]) -> Dimensions:
    checked: Dimensions = {}
    for key, value in dims.items():
        if not isinstance(key, str) or key == "":
            raise PiError(f"dimension keys for {name} must be non-empty strings")
        if not isinstance(value, int) or isinstance(value, bool):
            raise TypeError(f"dimension exponent for {name}.{key} must be int")
        checked[key] = value
    return canonicalize_dims(checked)


def _validate_exponents(exponents: Mapping[str, int]) -> dict[str, int]:
    checked: dict[str, int] = {}
    for name, value in exponents.items():
        if not isinstance(name, str) or name == "":
            raise PiError("product quantity names must be non-empty strings")
        if not isinstance(value, int) or isinstance(value, bool):
            raise TypeError(f"product exponent for {name} must be int")
        checked[name] = value
    return checked


def _dimension_matrix(quantities: Mapping[str, Dimensions]) -> list[list[int]]:
    rows = _dimension_rows(quantities)
    return [[quantities[name].get(row, 0) for name in quantities] for row in rows]


def _dimension_rows(quantities: Mapping[str, Dimensions]) -> tuple[str, ...]:
    keys = {key for dims in quantities.values() for key in dims}
    return tuple(sorted(keys, key=_dimension_sort_key))


def _dimension_sort_key(key: str) -> tuple[int, str]:
    try:
        return (DIM_ORDER.index(key), key)
    except ValueError:
        return (len(DIM_ORDER), key)


def _rank(matrix: list[list[int]]) -> int:
    _, pivots = _rref(matrix)
    return len(pivots)


def _rref(matrix: list[list[int]]) -> tuple[list[list[Fraction]], tuple[int, ...]]:
    if not matrix:
        return [], ()

    working = [[Fraction(value) for value in row] for row in matrix]
    row_count = len(working)
    col_count = len(working[0]) if row_count else 0
    pivot_row = 0
    pivots: list[int] = []

    for col in range(col_count):
        pivot = None
        for row in range(pivot_row, row_count):
            if working[row][col] != 0:
                pivot = row
                break
        if pivot is None:
            continue

        working[pivot_row], working[pivot] = working[pivot], working[pivot_row]
        pivot_value = working[pivot_row][col]
        working[pivot_row] = [value / pivot_value for value in working[pivot_row]]

        for row in range(row_count):
            if row == pivot_row:
                continue
            factor = working[row][col]
            if factor == 0:
                continue
            working[row] = [
                value - factor * pivot_value
                for value, pivot_value in zip(working[row], working[pivot_row])
            ]

        pivots.append(col)
        pivot_row += 1
        if pivot_row == row_count:
            break

    return working, tuple(pivots)


def _integer_nullspace_basis(matrix: list[list[int]], col_count: int) -> tuple[tuple[int, ...], ...]:
    if col_count == 0:
        return ()

    rref, pivots = _rref(matrix)
    pivot_set = set(pivots)
    free_cols = [col for col in range(col_count) if col not in pivot_set]
    groups: list[tuple[int, ...]] = []

    for free_col in free_cols:
        vector = [Fraction(0) for _ in range(col_count)]
        vector[free_col] = Fraction(1)
        for row, pivot_col in enumerate(pivots):
            vector[pivot_col] = -rref[row][free_col]
        groups.append(_normalize_integer_vector(vector))

    return tuple(groups)


def _normalize_integer_vector(vector: list[Fraction]) -> tuple[int, ...]:
    denominator_lcm = 1
    for value in vector:
        denominator_lcm = _lcm(denominator_lcm, value.denominator)

    integers = [int(value * denominator_lcm) for value in vector]
    nonzero = [abs(value) for value in integers if value != 0]
    if not nonzero:
        return tuple(integers)

    divisor = reduce(gcd, nonzero)
    integers = [value // divisor for value in integers]

    first_nonzero = next(value for value in integers if value != 0)
    if first_nonzero < 0:
        integers = [-value for value in integers]

    return tuple(integers)


def _lcm(left: int, right: int) -> int:
    return abs(left * right) // gcd(left, right)
