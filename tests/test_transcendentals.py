from __future__ import annotations

import pytest
import sympy as sp

from bridgman import DimensionalError, dims_of_expr


DIM_MAP = {
    "angle": {},
    "length": {"L": 1},
    "time": {"T": 1},
}


@pytest.mark.parametrize(
    "function",
    (sp.sin, sp.cos, sp.tan, sp.exp, sp.log, sp.sinh, sp.cosh, sp.tanh),
)
def test_transcendental_dimensionless_arg_returns_dimensionless(function) -> None:
    angle = sp.Symbol("angle")

    assert dims_of_expr(function(angle), DIM_MAP) == {}


@pytest.mark.parametrize(
    "function",
    (sp.sin, sp.cos, sp.tan, sp.exp, sp.log, sp.sinh, sp.cosh, sp.tanh),
)
def test_transcendental_dimensional_arg_raises_dimensional_error(function) -> None:
    length = sp.Symbol("length")

    with pytest.raises(DimensionalError, match=function.__name__):
        dims_of_expr(function(length), DIM_MAP)


def test_atan2_requires_two_dimensionless_args() -> None:
    angle = sp.Symbol("angle")
    length = sp.Symbol("length")

    assert dims_of_expr(sp.atan2(angle, angle), DIM_MAP) == {}
    with pytest.raises(DimensionalError, match="atan2"):
        dims_of_expr(sp.atan2(length, angle), DIM_MAP)


@pytest.mark.parametrize(
    "expr",
    (
        sp.Derivative(sp.Symbol("length"), sp.Symbol("time")),
        sp.Integral(sp.Symbol("length"), sp.Symbol("time")),
        sp.Piecewise(
            (sp.Symbol("length"), sp.Symbol("condition", boolean=True)),
            evaluate=False,
        ),
        sp.Min(sp.Symbol("length"), sp.Symbol("time")),
        sp.Max(sp.Symbol("length"), sp.Symbol("time")),
        sp.Abs(sp.Symbol("length")),
        sp.KroneckerDelta(sp.Symbol("length"), sp.Symbol("time")),
    ),
)
def test_unsupported_nodes_raise_dimensional_error(expr) -> None:
    with pytest.raises(DimensionalError):
        dims_of_expr(expr, DIM_MAP)


def test_nested_eq_raises_dimensional_error() -> None:
    angle = sp.Symbol("angle")

    with pytest.raises(DimensionalError, match="Nested Eq"):
        dims_of_expr(sp.Eq(angle, sp.Eq(angle, angle)), DIM_MAP)
