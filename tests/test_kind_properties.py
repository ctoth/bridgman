from __future__ import annotations

from hypothesis import assume
from hypothesis import given
from hypothesis import strategies as st

from bridgman import (
    DuplicateOperationRuleError,
    KindRegistry,
    OperationRule,
    QuantityKind,
    UnknownKindError,
    canonicalize_dims,
    dims_equal,
    div_dims,
    mul_dims,
)


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
    max_size=16,
).filter(lambda name: name.strip() != "")

unique_kind_names = st.lists(kind_names, min_size=3, max_size=6, unique=True)


@given(unique_kind_names, st.lists(dimension_maps, min_size=3, max_size=6))
def test_registry_lookup_is_independent_of_kind_input_order(
    names: list[str],
    dims_list: list[dict[str, int]],
) -> None:
    assume(len(names) == len(dims_list))
    forward_kinds = [
        QuantityKind(name, dims) for name, dims in zip(names, dims_list, strict=True)
    ]
    reverse_kinds = list(reversed(forward_kinds))

    forward = KindRegistry(kinds=forward_kinds)
    reverse = KindRegistry(kinds=reverse_kinds)

    for kind in forward_kinds:
        assert forward.kind_dimensions(kind.name) == reverse.kind_dimensions(kind.name)


@given(
    kind_names,
    kind_names,
    kind_names,
    dimension_maps,
    dimension_maps,
    st.sampled_from(("mul", "div")),
)
def test_accepted_generated_rules_satisfy_dimension_arithmetic(
    left_name: str,
    right_name: str,
    result_name: str,
    left_dims: dict[str, int],
    right_dims: dict[str, int],
    op: str,
) -> None:
    assume(len({left_name, right_name, result_name}) == 3)
    result_dims = mul_dims(left_dims, right_dims) if op == "mul" else div_dims(left_dims, right_dims)
    registry = KindRegistry(
        kinds=[
            QuantityKind(left_name, left_dims),
            QuantityKind(right_name, right_dims),
            QuantityKind(result_name, result_dims),
        ],
        rules=[OperationRule(left_name, op, right_name, result_name)],
    )

    assert registry.result_kind(left_name, op, right_name) == result_name
    assert dims_equal(registry.kind_dimensions(result_name), result_dims)


@given(kind_names, kind_names, kind_names, dimension_maps, dimension_maps)
def test_generated_duplicate_operation_keys_are_rejected(
    left_name: str,
    right_name: str,
    result_name: str,
    left_dims: dict[str, int],
    right_dims: dict[str, int],
) -> None:
    assume(len({left_name, right_name, result_name}) == 3)
    result_dims = mul_dims(left_dims, right_dims)

    try:
        KindRegistry(
            kinds=[
                QuantityKind(left_name, left_dims),
                QuantityKind(right_name, right_dims),
                QuantityKind(result_name, result_dims),
            ],
            rules=[
                OperationRule(left_name, "mul", right_name, result_name),
                OperationRule(left_name, "mul", right_name, result_name),
            ],
        )
    except DuplicateOperationRuleError:
        return

    raise AssertionError("duplicate operation rule was accepted")


@given(kind_names, kind_names, kind_names, dimension_maps, dimension_maps)
def test_generated_unknown_rule_references_are_rejected(
    left_name: str,
    right_name: str,
    result_name: str,
    left_dims: dict[str, int],
    right_dims: dict[str, int],
) -> None:
    assume(len({left_name, right_name, result_name}) == 3)

    try:
        KindRegistry(
            kinds=[
                QuantityKind(left_name, left_dims),
                QuantityKind(right_name, right_dims),
            ],
            rules=[OperationRule(left_name, "mul", right_name, result_name)],
        )
    except UnknownKindError:
        return

    raise AssertionError("unknown result kind reference was accepted")


@given(unique_kind_names, st.lists(dimension_maps, min_size=3, max_size=6), dimension_maps)
def test_kinds_with_dimensions_returns_exact_generated_matches(
    names: list[str],
    dims_list: list[dict[str, int]],
    target_dims: dict[str, int],
) -> None:
    assume(len(names) == len(dims_list))
    registry = KindRegistry(
        kinds=[QuantityKind(name, dims) for name, dims in zip(names, dims_list, strict=True)]
    )
    expected = tuple(
        name
        for name, dims in zip(names, dims_list, strict=True)
        if canonicalize_dims(dims) == canonicalize_dims(target_dims)
    )

    assert registry.kinds_with_dimensions(target_dims) == expected
