from __future__ import annotations

from collections.abc import Callable

import pytest
import sympy as sp
from hypothesis import given
from hypothesis import strategies as st

from bridgman import DimensionalError, canonicalize_dims, dims_equal, dims_of_expr, verify_expr


DIM_KEYS = ("M", "L", "T", "I", "Theta", "N", "J")

dimension_maps = st.dictionaries(
    st.sampled_from(DIM_KEYS),
    st.integers(min_value=-4, max_value=4),
    max_size=len(DIM_KEYS),
)

dimensioned_maps = dimension_maps.filter(lambda dims: canonicalize_dims(dims) != {})

mixed_dimension_pairs = st.tuples(dimension_maps, dimension_maps).filter(
    lambda pair: not dims_equal(pair[0], pair[1])
)


def test_dimensionless_base_accepts_symbolic_and_float_exponents() -> None:
    x = sp.Symbol("x")
    u = sp.Symbol("u")

    assert dims_of_expr(2**x, {}) == {}
    assert dims_of_expr(u ** sp.Float("0.5"), {"u": {}}) == {}


def test_dimensioned_base_still_rejects_symbolic_exponent() -> None:
    x = sp.Symbol("x")
    n = sp.Symbol("n")

    with pytest.raises(DimensionalError, match="non-numeric exponent"):
        dims_of_expr(x**n, {"x": {"L": 1}})


def test_atan2_accepts_equal_dimensioned_arguments() -> None:
    y = sp.Symbol("y")
    x = sp.Symbol("x")

    assert dims_of_expr(sp.atan2(y, x), {"y": {"L": 1}, "x": {"L": 1}}) == {}


def test_atan2_rejects_mixed_dimension_arguments() -> None:
    y = sp.Symbol("y")
    x = sp.Symbol("x")

    with pytest.raises(DimensionalError):
        dims_of_expr(sp.atan2(y, x), {"y": {"L": 1}, "x": {"T": 1}})


@given(dimension_maps)
def test_abs_preserves_generated_dimensions(dims: dict[str, int]) -> None:
    x = sp.Symbol("x")

    assert dims_equal(dims_of_expr(sp.Abs(x), {"x": dims}), dims)


@given(dimension_maps, st.sampled_from((sp.Min, sp.Max)))
def test_min_max_accept_generated_same_dimensions(
    dims: dict[str, int],
    function: Callable[..., sp.Expr],
) -> None:
    x = sp.Symbol("x")
    y = sp.Symbol("y")

    result = dims_of_expr(function(x, y, evaluate=False), {"x": dims, "y": dims})

    assert dims_equal(result, dims)


@given(mixed_dimension_pairs, st.sampled_from((sp.Min, sp.Max)))
def test_min_max_reject_generated_mixed_dimensions(
    pair: tuple[dict[str, int], dict[str, int]],
    function: Callable[..., sp.Expr],
) -> None:
    x = sp.Symbol("x")
    y = sp.Symbol("y")

    with pytest.raises(DimensionalError):
        dims_of_expr(function(x, y, evaluate=False), {"x": pair[0], "y": pair[1]})


@given(dimension_maps, dimension_maps)
def test_atan2_accepts_generated_operands_iff_dimensions_are_equal(
    left_dims: dict[str, int],
    right_dims: dict[str, int],
) -> None:
    y = sp.Symbol("y")
    x = sp.Symbol("x")
    expr = sp.atan2(y, x)
    dim_map = {"y": left_dims, "x": right_dims}

    if dims_equal(left_dims, right_dims):
        assert dims_of_expr(expr, dim_map) == {}
    else:
        with pytest.raises(DimensionalError):
            dims_of_expr(expr, dim_map)


@given(
    dimension_maps,
    dimension_maps,
    st.sampled_from((sp.Lt, sp.Le, sp.Gt, sp.Ge)),
)
def test_inequalities_accept_generated_operands_iff_dimensions_are_equal(
    left_dims: dict[str, int],
    right_dims: dict[str, int],
    relation: Callable[..., sp.Expr],
) -> None:
    left = sp.Symbol("left")
    right = sp.Symbol("right")
    expr = relation(left, right, evaluate=False)
    dim_map = {"left": left_dims, "right": right_dims}

    assert verify_expr(expr, dim_map) is dims_equal(left_dims, right_dims)
