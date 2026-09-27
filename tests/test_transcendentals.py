from __future__ import annotations

import pytest
import sympy as sp

from bridgman import DimensionError, UnsupportedExpressionError, dims_of_expr


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
def test_transcendental_dimensional_arg_is_the_cores_refusal(function) -> None:
    length = sp.Symbol("length")

    with pytest.raises(DimensionError.NotDimensionless) as refused:
        dims_of_expr(function(length), DIM_MAP)
    assert refused.value.fields == {"dimensions": {"L": "1"}}


def test_atan2_is_a_transcendental_of_the_ratio() -> None:
    angle = sp.Symbol("angle")
    length = sp.Symbol("length")

    assert dims_of_expr(sp.atan2(angle, angle), DIM_MAP) == {}
    assert dims_of_expr(sp.atan2(length, length), DIM_MAP) == {}
    with pytest.raises(DimensionError.NotDimensionless):
        dims_of_expr(sp.atan2(length, angle), DIM_MAP)


@pytest.mark.parametrize("function", (sp.Min, sp.Max))
def test_min_max_of_unequal_dimensions_is_the_cores_refusal(function) -> None:
    with pytest.raises(DimensionError.Unequal):
        dims_of_expr(function(sp.Symbol("length"), sp.Symbol("time")), DIM_MAP)


@pytest.mark.parametrize(
    "expr",
    (
        sp.Derivative(sp.Symbol("length"), sp.Symbol("time")),
        sp.Integral(sp.Symbol("length"), sp.Symbol("time")),
        sp.Piecewise(
            (sp.Symbol("length"), sp.Symbol("condition", boolean=True)),
            evaluate=False,
        ),
        sp.KroneckerDelta(sp.Symbol("length"), sp.Symbol("time")),
    ),
)
def test_unsupported_nodes_are_refused_by_the_walker(expr) -> None:
    with pytest.raises(UnsupportedExpressionError):
        dims_of_expr(expr, DIM_MAP)


def test_nested_eq_is_refused_by_the_walker() -> None:
    angle = sp.Symbol("angle")

    with pytest.raises(UnsupportedExpressionError, match="Nested relational"):
        dims_of_expr(sp.Eq(angle, sp.Eq(angle, angle)), DIM_MAP)
