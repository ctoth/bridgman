from __future__ import annotations

import sympy as sp
from hypothesis import assume
from hypothesis import given
from hypothesis import strategies as st

from bridgman import (
    KindRegistry,
    OperationRule,
    QuantityKind,
    div_dims,
    explain_expr,
    explain_expr_kinds,
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


@given(dimension_maps, dimension_maps)
def test_explain_expr_ok_matches_verify_expr(
    left_dims: dict[str, int],
    right_dims: dict[str, int],
) -> None:
    left = sp.Symbol("left")
    right = sp.Symbol("right")
    equation = sp.Eq(left, right)
    dim_map = {"left": left_dims, "right": right_dims}

    assert explain_expr(equation, dim_map).ok == verify_expr(equation, dim_map)


@given(
    kind_names,
    kind_names,
    kind_names,
    dimension_maps,
    dimension_maps,
    st.sampled_from(("mul", "div")),
)
def test_explain_expr_kinds_ok_matches_verify_expr_kinds_for_valid_binary_rules(
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
    lhs = sp.Symbol("lhs")
    left = sp.Symbol("left")
    right = sp.Symbol("right")
    rhs = left * right if op == "mul" else left / right
    equation = sp.Eq(lhs, rhs)
    kind_map = {"lhs": result_kind, "left": left_kind, "right": right_kind}

    assert explain_expr_kinds(equation, registry=registry, kind_map=kind_map).ok == verify_expr_kinds(
        equation,
        registry=registry,
        kind_map=kind_map,
    )


@given(kind_names, kind_names, dimension_maps, dimension_maps)
def test_explain_expr_kinds_generated_missing_rule_failures_have_concrete_reason(
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
    result = explain_expr_kinds(
        sp.Eq(left, left * right),
        registry=registry,
        kind_map={"left": left_kind, "right": right_kind},
    )

    assert not result.ok
    assert result.reason
    assert result.steps
