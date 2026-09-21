"""Buckingham Pi helpers for dimensionless power products."""

from __future__ import annotations

from collections.abc import Mapping
from bridgman._core import count_pi_groups, pi_groups
from bridgman.dimensions import Dimensions, canonicalize_dims, is_dimensionless


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

