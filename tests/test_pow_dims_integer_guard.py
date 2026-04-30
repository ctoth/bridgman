from __future__ import annotations

import pytest

from bridgman import pow_dims


def test_pow_dims_rejects_non_integer_exponent() -> None:
    with pytest.raises(TypeError):
        pow_dims({"L": 2}, 0.5)  # type: ignore[arg-type]


def test_pow_dims_rejects_bool_exponent() -> None:
    with pytest.raises(TypeError):
        pow_dims({"L": 2}, True)  # type: ignore[arg-type]


def test_pow_dims_integer_exponent_multiplies_exponents() -> None:
    assert pow_dims({"L": 2}, 3) == {"L": 6}


def test_pow_dims_zero_exponent_is_dimensionless() -> None:
    assert pow_dims({"L": 2, "T": -1}, 0) == {}
