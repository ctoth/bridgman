from __future__ import annotations

import sympy as sp
from hypothesis import assume
from hypothesis import given
from hypothesis import strategies as st

from bridgman import (
    KindRegistry,
    OperationRule,
    QuantityKind,
    explain_expr_kinds,
    verify_expr_kinds,
)


DIM_KEYS = ("M", "L", "T", "I", "Theta", "N", "J")

dimension_maps = st.dictionaries(
    st.sampled_from(DIM_KEYS),
    st.integers(min_value=-3, max_value=3),
    max_size=len(DIM_KEYS),
)

propstore_kind_labels = st.text(
    alphabet=st.characters(
        blacklist_categories=("Cs",),
        blacklist_characters=("\x00",),
    ),
    min_size=1,
    max_size=24,
).filter(lambda name: name.strip() != "")


@given(propstore_kind_labels, dimension_maps)
def test_propstore_style_kind_labels_round_trip_through_registry_lookup(
    kind_label: str,
    dims: dict[str, int],
) -> None:
    registry = KindRegistry(kinds=[QuantityKind(kind_label, dims)])

    assert registry.kind_dimensions(kind_label) == QuantityKind(kind_label, dims).dimensions


@given(propstore_kind_labels, propstore_kind_labels, propstore_kind_labels, dimension_maps, dimension_maps)
def test_alpha_renaming_symbols_preserves_kind_aware_verification(
    left_kind: str,
    right_kind: str,
    result_kind: str,
    left_dims: dict[str, int],
    right_dims: dict[str, int],
) -> None:
    assume(len({left_kind, right_kind, result_kind}) == 3)
    result_dims = {key: left_dims.get(key, 0) + right_dims.get(key, 0) for key in DIM_KEYS}
    registry = KindRegistry(
        kinds=[
            QuantityKind(left_kind, left_dims),
            QuantityKind(right_kind, right_dims),
            QuantityKind(result_kind, result_dims),
        ],
        rules=[OperationRule(left_kind, "mul", right_kind, result_kind)],
    )

    a, b, c = sp.symbols("a b c")
    x, y, z = sp.symbols("x y z")

    assert verify_expr_kinds(
        sp.Eq(c, a * b),
        registry=registry,
        kind_map={"a": left_kind, "b": right_kind, "c": result_kind},
    ) == verify_expr_kinds(
        sp.Eq(z, x * y),
        registry=registry,
        kind_map={"x": left_kind, "y": right_kind, "z": result_kind},
    )


@given(propstore_kind_labels, propstore_kind_labels, dimension_maps)
def test_changing_only_concept_kind_label_changes_kind_aware_verification(
    original_kind: str,
    renamed_kind: str,
    dims: dict[str, int],
) -> None:
    assume(original_kind != renamed_kind)
    registry = KindRegistry(kinds=[QuantityKind(original_kind, dims)])
    x = sp.Symbol("x")

    result = explain_expr_kinds(
        sp.Eq(x, x, evaluate=False),
        registry=registry,
        kind_map={"x": renamed_kind},
    )

    assert not result.ok
    assert renamed_kind in result.reason
