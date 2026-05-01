from __future__ import annotations

import pytest
import sympy as sp
from hypothesis import assume
from hypothesis import given
from hypothesis import strategies as st

from bridgman import (
    KindMismatchError,
    KindRegistry,
    MissingOperationRuleError,
    OperationRule,
    QuantityKind,
    dims_equal,
    dims_of_expr,
    div_dims,
    kind_of_expr,
    mul_dims,
    verify_expr,
    verify_expr_kinds,
)


DIM_KEYS = ("M", "L", "T", "I", "Theta", "N", "J")

dimension_maps = st.dictionaries(
    st.sampled_from(DIM_KEYS),
    st.integers(min_value=-3, max_value=3),
    max_size=len(DIM_KEYS),
)

kind_names = st.text(
    alphabet=st.characters(
        blacklist_categories=("Cs",),
        blacklist_characters=("\x00",),
    ),
    min_size=1,
    max_size=16,
).filter(lambda name: name.strip() != "")


@given(
    kind_names,
    kind_names,
    kind_names,
    dimension_maps,
    dimension_maps,
    st.sampled_from(("mul", "div")),
)
def test_kind_inference_dimensions_agree_with_dimension_only_for_valid_binary_rules(
    left_kind: str,
    right_kind: str,
    result_kind: str,
    left_dims: dict[str, int],
    right_dims: dict[str, int],
    op: str,
) -> None:
    assume(len({left_kind, right_kind, result_kind}) == 3)
    result_dims = mul_dims(left_dims, right_dims) if op == "mul" else div_dims(left_dims, right_dims)
    registry = KindRegistry(
        kinds=[
            QuantityKind(left_kind, left_dims),
            QuantityKind(right_kind, right_dims),
            QuantityKind(result_kind, result_dims),
        ],
        rules=[OperationRule(left_kind, op, right_kind, result_kind)],
    )
    left = sp.Symbol("left")
    right = sp.Symbol("right")
    expr = left * right if op == "mul" else left / right

    inferred_kind = kind_of_expr(
        expr,
        registry=registry,
        kind_map={"left": left_kind, "right": right_kind},
    )

    assert inferred_kind == result_kind
    assert dims_equal(
        registry.kind_dimensions(inferred_kind),
        dims_of_expr(expr, {"left": left_dims, "right": right_dims}),
    )


@given(kind_names, kind_names, dimension_maps, dimension_maps)
def test_generated_missing_operation_edges_fail_closed(
    left_kind: str,
    right_kind: str,
    left_dims: dict[str, int],
    right_dims: dict[str, int],
) -> None:
    assume(left_kind != right_kind)
    registry = KindRegistry(
        kinds=[QuantityKind(left_kind, left_dims), QuantityKind(right_kind, right_dims)]
    )
    left = sp.Symbol("left")
    right = sp.Symbol("right")

    with pytest.raises(MissingOperationRuleError):
        kind_of_expr(
            left * right,
            registry=registry,
            kind_map={"left": left_kind, "right": right_kind},
        )


@given(kind_names, kind_names, dimension_maps)
def test_generated_same_dimension_different_kind_additions_fail(
    left_kind: str,
    right_kind: str,
    dims: dict[str, int],
) -> None:
    assume(left_kind != right_kind)
    registry = KindRegistry(
        kinds=[QuantityKind(left_kind, dims), QuantityKind(right_kind, dims)]
    )
    left = sp.Symbol("left")
    right = sp.Symbol("right")

    with pytest.raises(KindMismatchError):
        kind_of_expr(
            left + right,
            registry=registry,
            kind_map={"left": left_kind, "right": right_kind},
        )


@given(kind_names, kind_names, dimension_maps)
def test_kind_aware_rejection_is_stricter_than_dimension_only_for_generated_twins(
    left_kind: str,
    right_kind: str,
    dims: dict[str, int],
) -> None:
    assume(left_kind != right_kind)
    registry = KindRegistry(
        kinds=[QuantityKind(left_kind, dims), QuantityKind(right_kind, dims)]
    )
    left = sp.Symbol("left")
    right = sp.Symbol("right")
    equation = sp.Eq(left, right)

    assert verify_expr(equation, {"left": dims, "right": dims})
    assert not verify_expr_kinds(
        equation,
        registry=registry,
        kind_map={"left": left_kind, "right": right_kind},
    )
