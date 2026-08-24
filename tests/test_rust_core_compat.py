from __future__ import annotations

import builtins
import sys

import bridgman
import pytest
from bridgman import _core


def test_public_arithmetic_delegates_to_core_with_large_exponents() -> None:
    huge = 10**100
    left = {"L": huge, "T": -2}
    right = {"L": huge, "T": 2}

    assert bridgman.mul_dims(left, right) == _core.mul_dims(left, right) == {
        "L": 2 * huge
    }
    assert bridgman.div_dims(left, right) == _core.div_dims(left, right) == {
        "T": -4
    }
    assert bridgman.pow_dims({"M": huge}, -huge) == {"M": -(huge**2)}


def test_core_canonicalizes_theta_and_strips_zeroes() -> None:
    upper_theta = chr(0x0398)
    lower_theta = chr(0x03B8)

    assert _core.canonicalize_dims(
        {upper_theta: 4, lower_theta: -1, "Theta": -3, "L": 0}
    ) == {}
    assert _core.dims_signature({upper_theta: 2, "T": 0}) == "Theta:2"
    assert _core.parse_dims_signature("Theta:2,T:0") == {"Theta": 2}


def test_core_formatting_matches_public_facade() -> None:
    dimensions = {"custom": 3, "T": -(10**20), "M": 1, "L": 0}
    expected = "M T⁻¹" + "⁰" * 20 + " custom³"

    assert _core.format_dims(dimensions) == bridgman.format_dims(dimensions) == expected


def test_core_has_no_sympy_import_dependency(monkeypatch) -> None:
    original_import = builtins.__import__

    def import_without_sympy(name, globals=None, locals=None, fromlist=(), level=0):
        if name == "sympy" or name.startswith("sympy."):
            raise ModuleNotFoundError("No module named 'sympy'", name="sympy")
        return original_import(name, globals, locals, fromlist, level)

    monkeypatch.setattr(builtins, "__import__", import_without_sympy)
    assert _core.dims_signature({"L": 1}) == "L:1"


@pytest.mark.skipif(
    not hasattr(sys, "set_int_max_str_digits"),
    reason="CPython integer string conversion limits require Python 3.11+",
)
def test_public_formatting_and_parsing_honor_configured_int_string_limit() -> None:
    original_limit = sys.get_int_max_str_digits()
    try:
        sys.set_int_max_str_digits(640)
        huge = 10**700

        with pytest.raises(ValueError):
            bridgman.format_dims({"L": huge})
        with pytest.raises(ValueError):
            bridgman.dims_signature({"L": huge})
        with pytest.raises(ValueError):
            bridgman.parse_dims_signature("L:" + "1" * 700)
    finally:
        sys.set_int_max_str_digits(original_limit)


@pytest.mark.skipif(
    not hasattr(sys, "set_int_max_str_digits"),
    reason="CPython integer string conversion limits require Python 3.11+",
)
def test_public_formatting_and_parsing_support_unlimited_int_strings() -> None:
    original_limit = sys.get_int_max_str_digits()
    try:
        sys.set_int_max_str_digits(0)
        huge = 10**5000
        decimal = "1" + "0" * 5000

        assert bridgman.format_dims({"L": huge}) == "L¹" + "⁰" * 5000
        assert bridgman.dims_signature({"L": huge}) == f"L:{decimal}"
        assert bridgman.parse_dims_signature(f"L:{decimal}") == {"L": huge}
    finally:
        sys.set_int_max_str_digits(original_limit)


@pytest.mark.parametrize("signature", ["", "L", "L:not-an-int", "L:1:2"])
def test_invalid_signatures_raise_value_error(signature: str) -> None:
    with pytest.raises(ValueError):
        bridgman.parse_dims_signature(signature)
    with pytest.raises(ValueError):
        _core.parse_dims_signature(signature)


def test_public_parser_preserves_python_integer_string_syntax() -> None:
    assert bridgman.parse_dims_signature("L: +1_000,T:-2") == {"L": 1000, "T": -2}
