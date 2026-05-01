from __future__ import annotations

import sympy as sp
from hypothesis import assume
from hypothesis import given
from hypothesis import strategies as st

from bridgman import canonicalize_dims, dims_equal, verify_expr


DIM_KEYS = ("M", "L", "T", "I", "Theta", "N", "J")

dimension_maps = st.dictionaries(
    st.sampled_from(DIM_KEYS),
    st.integers(min_value=-4, max_value=4),
    max_size=len(DIM_KEYS),
)

kind_names = st.text(
    alphabet=st.characters(
        blacklist_categories=("Cs",),
        blacklist_characters=("\x00",),
    ),
    min_size=1,
    max_size=24,
).filter(lambda name: name.strip() != "")


@given(kind_names, kind_names, dimension_maps)
def test_dimension_only_equality_cannot_distinguish_generated_dimensional_twins(
    left_kind: str,
    right_kind: str,
    dims: dict[str, int],
) -> None:
    assume(left_kind != right_kind)
    left = sp.Symbol("left")
    right = sp.Symbol("right")

    assert verify_expr(sp.Eq(left, right), {"left": dims, "right": dims})


@given(kind_names, kind_names, dimension_maps)
def test_generated_dimensional_twins_are_distinct_when_kind_labels_are_considered(
    left_kind: str,
    right_kind: str,
    dims: dict[str, int],
) -> None:
    assume(left_kind != right_kind)

    assert canonicalize_dims(dims) == canonicalize_dims(dims)
    assert dims_equal(dims, dims)
    assert left_kind != right_kind
