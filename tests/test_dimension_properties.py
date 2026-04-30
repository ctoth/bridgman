from __future__ import annotations

from collections.abc import Callable

import pytest
import sympy as sp
from hypothesis import given
from hypothesis import strategies as st

from bridgman import (
    DimensionalError,
    canonicalize_dims,
    dims_equal,
    dims_of_expr,
    dims_signature,
    div_dims,
    mul_dims,
    parse_dims_signature,
    pow_dims,
)


DIM_KEYS = ("M", "L", "T", "I", "Theta", "N", "J")


dimension_maps = st.dictionaries(
    st.sampled_from(DIM_KEYS),
    st.integers(min_value=-4, max_value=4),
    max_size=len(DIM_KEYS),
)

dimensioned_maps = dimension_maps.filter(
    lambda dims: canonicalize_dims(dims) != {}
)


@given(dimension_maps, dimension_maps)
def test_mul_dims_is_commutative_under_dimensional_equality(
    left: dict[str, int],
    right: dict[str, int],
) -> None:
    assert dims_equal(mul_dims(left, right), mul_dims(right, left))


@given(dimension_maps, dimension_maps)
def test_division_cancels_multiplication(
    left: dict[str, int],
    right: dict[str, int],
) -> None:
    assert dims_equal(div_dims(mul_dims(left, right), right), left)


@given(dimension_maps, dimension_maps, st.integers(min_value=-4, max_value=4))
def test_power_distributes_over_multiplication(
    left: dict[str, int],
    right: dict[str, int],
    exponent: int,
) -> None:
    assert dims_equal(
        pow_dims(mul_dims(left, right), exponent),
        mul_dims(pow_dims(left, exponent), pow_dims(right, exponent)),
    )


@given(
    st.lists(
        st.tuples(st.sampled_from(DIM_KEYS), st.integers(-4, 4)),
        max_size=len(DIM_KEYS),
        unique_by=lambda entry: entry[0],
    )
)
def test_canonical_dimension_signatures_ignore_input_order(
    entries: list[tuple[str, int]],
) -> None:
    forward = dict(entries)
    reverse = dict(reversed(entries))

    assert dims_signature(forward) == dims_signature(reverse)
    assert parse_dims_signature(dims_signature(forward)) == canonicalize_dims(forward)


@given(st.sampled_from((sp.sin, sp.cos, sp.tan, sp.exp, sp.log, sp.sinh, sp.cosh, sp.tanh)))
def test_transcendentals_accept_only_dimensionless_generated_arguments(
    function: Callable[[sp.Expr], sp.Expr],
) -> None:
    symbol = sp.Symbol("x")

    assert dims_of_expr(function(symbol), {"x": {}}) == {}


@given(
    st.sampled_from((sp.sin, sp.cos, sp.tan, sp.exp, sp.log, sp.sinh, sp.cosh, sp.tanh)),
    dimensioned_maps,
)
def test_transcendentals_reject_generated_dimensioned_arguments(
    function: Callable[[sp.Expr], sp.Expr],
    dims: dict[str, int],
) -> None:
    symbol = sp.Symbol("x")

    with pytest.raises(DimensionalError):
        dims_of_expr(function(symbol), {"x": dims})
