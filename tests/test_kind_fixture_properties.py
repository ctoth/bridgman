from __future__ import annotations

import pytest
from hypothesis import assume
from hypothesis import given
from hypothesis import strategies as st

from bridgman import (
    DuplicateKindError,
    InvalidOperationRuleError,
    KindRegistry,
    OperationRule,
    QuantityKind,
    UnknownKindError,
    div_dims,
    mul_dims,
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


@given(kind_names, kind_names, kind_names, dimension_maps, dimension_maps, st.sampled_from(("mul", "div")))
def test_generated_valid_fixture_fragments_load_into_equivalent_registries(
    left_name: str,
    right_name: str,
    result_name: str,
    left_dims: dict[str, int],
    right_dims: dict[str, int],
    op: str,
) -> None:
    assume(len({left_name, right_name, result_name}) == 3)
    result_dims = mul_dims(left_dims, right_dims) if op == "mul" else div_dims(left_dims, right_dims)
    kinds = [
        QuantityKind(left_name, left_dims),
        QuantityKind(right_name, right_dims),
        QuantityKind(result_name, result_dims),
    ]
    rule = OperationRule(left_name, op, right_name, result_name)

    forward = KindRegistry(kinds=kinds, rules=[rule])
    reverse = KindRegistry(kinds=list(reversed(kinds)), rules=[rule])

    assert forward.result_kind(left_name, op, right_name) == reverse.result_kind(
        left_name,
        op,
        right_name,
    )


@given(kind_names, dimension_maps)
def test_generated_duplicate_fixture_kinds_fail_closed(name: str, dims: dict[str, int]) -> None:
    with pytest.raises(DuplicateKindError):
        KindRegistry(kinds=[QuantityKind(name, dims), QuantityKind(name, dims)])


@given(kind_names, kind_names, kind_names, dimension_maps, dimension_maps)
def test_generated_unknown_fixture_rule_references_fail_closed(
    left_name: str,
    right_name: str,
    result_name: str,
    left_dims: dict[str, int],
    right_dims: dict[str, int],
) -> None:
    assume(len({left_name, right_name, result_name}) == 3)

    with pytest.raises(UnknownKindError):
        KindRegistry(
            kinds=[QuantityKind(left_name, left_dims), QuantityKind(right_name, right_dims)],
            rules=[OperationRule(left_name, "mul", right_name, result_name)],
        )


@given(kind_names, kind_names, kind_names, dimension_maps, dimension_maps, dimension_maps)
def test_generated_dimensionally_invalid_fixture_rules_fail_closed(
    left_name: str,
    right_name: str,
    result_name: str,
    left_dims: dict[str, int],
    right_dims: dict[str, int],
    invalid_result_dims: dict[str, int],
) -> None:
    assume(len({left_name, right_name, result_name}) == 3)
    assume(invalid_result_dims != mul_dims(left_dims, right_dims))

    with pytest.raises(InvalidOperationRuleError):
        KindRegistry(
            kinds=[
                QuantityKind(left_name, left_dims),
                QuantityKind(right_name, right_dims),
                QuantityKind(result_name, invalid_result_dims),
            ],
            rules=[OperationRule(left_name, "mul", right_name, result_name)],
        )
