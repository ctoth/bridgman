from __future__ import annotations

from functools import reduce
from math import gcd

import pytest
import sympy as sp
from hypothesis import given
from hypothesis import strategies as st

from bridgman import (
    KindRegistry,
    OperationRule,
    PiError,
    QuantityKind,
    count_pi_groups,
    is_dimensionless_product,
    pi_groups,
    verify_expr_kinds,
)


MASS = {"M": 1}
LENGTH = {"L": 1}
TIME = {"T": 1}
VELOCITY = {"L": 1, "T": -1}
DENSITY = {"M": 1, "L": -3}
DYNAMIC_VISCOSITY = {"M": 1, "L": -1, "T": -1}
FORCE = {"M": 1, "L": 1, "T": -2}
ACCELERATION = {"L": 1, "T": -2}
ENERGY = {"M": 1, "L": 2, "T": -2}


def reynolds_quantities() -> dict[str, dict[str, int]]:
    return {
        "rho": DENSITY,
        "v": VELOCITY,
        "L": LENGTH,
        "mu": DYNAMIC_VISCOSITY,
    }


def test_reynolds_product_is_dimensionless() -> None:
    assert is_dimensionless_product(
        reynolds_quantities(),
        {"rho": 1, "v": 1, "L": 1, "mu": -1},
    )


def test_reynolds_without_density_is_not_dimensionless() -> None:
    assert not is_dimensionless_product(
        {"v": VELOCITY, "L": LENGTH, "mu": DYNAMIC_VISCOSITY},
        {"v": 1, "L": 1, "mu": -1},
    )


def test_unknown_product_names_raise_pi_error() -> None:
    with pytest.raises(PiError, match="unknown quantity"):
        is_dimensionless_product({"v": VELOCITY}, {"rho": 1})


@pytest.mark.parametrize("exponent", [True, 1.5, "1"])
def test_non_integer_product_exponents_raise_type_error(exponent: object) -> None:
    with pytest.raises(TypeError, match="exponent"):
        is_dimensionless_product({"v": VELOCITY}, {"v": exponent})


def test_propstore_concept_id_labels_are_opaque() -> None:
    assert is_dimensionless_product(
        {
            "ps:concept:density": DENSITY,
            "ps:concept:velocity": VELOCITY,
            "ps:concept:length": LENGTH,
            "ps:concept:dynamic-viscosity": DYNAMIC_VISCOSITY,
        },
        {
            "ps:concept:density": 1,
            "ps:concept:velocity": 1,
            "ps:concept:length": 1,
            "ps:concept:dynamic-viscosity": -1,
        },
    )


@pytest.mark.parametrize(
    ("quantities", "expected"),
    [
        (
            {
                "t": TIME,
                "l": LENGTH,
                "g": ACCELERATION,
                "m": MASS,
                "theta": {},
            },
            2,
        ),
        (reynolds_quantities(), 1),
        ({"F": FORCE, "m": MASS, "a": ACCELERATION}, 1),
        ({"M": MASS, "L": LENGTH, "T": TIME}, 0),
        ({}, 0),
        ({"theta": {}, "strain": {}}, 2),
    ],
)
def test_count_pi_groups_examples(
    quantities: dict[str, dict[str, int]],
    expected: int,
) -> None:
    assert count_pi_groups(quantities) == expected


DIM_KEYS = ("M", "L", "T", "I", "Theta", "N", "J")

dimension_maps = st.dictionaries(
    st.sampled_from(DIM_KEYS),
    st.integers(min_value=-3, max_value=3),
    max_size=len(DIM_KEYS),
)

quantity_entries = st.lists(
    st.tuples(st.text(min_size=1), dimension_maps),
    min_size=0,
    max_size=7,
    unique_by=lambda entry: entry[0],
)


@given(quantity_entries)
def test_count_pi_groups_is_invariant_under_quantity_order(
    entries: list[tuple[str, dict[str, int]]],
) -> None:
    assert count_pi_groups(dict(entries)) == count_pi_groups(dict(reversed(entries)))


@given(quantity_entries)
def test_count_pi_groups_is_bounded(
    entries: list[tuple[str, dict[str, int]]],
) -> None:
    count = count_pi_groups(dict(entries))

    assert 0 <= count <= len(entries)


def test_reynolds_basis_contains_checked_reynolds_product() -> None:
    groups = pi_groups(reynolds_quantities())

    assert groups == ({"rho": 1, "v": 1, "L": 1, "mu": -1},)
    assert all(is_dimensionless_product(reynolds_quantities(), group) for group in groups)


@given(quantity_entries)
def test_generated_groups_are_dimensionless(
    entries: list[tuple[str, dict[str, int]]],
) -> None:
    quantities = dict(entries)

    for group in pi_groups(quantities):
        assert is_dimensionless_product(quantities, group)


@given(quantity_entries)
def test_generated_group_count_matches_canonical_count(
    entries: list[tuple[str, dict[str, int]]],
) -> None:
    quantities = dict(entries)

    assert len(pi_groups(quantities)) == count_pi_groups(quantities)


@given(quantity_entries)
def test_generated_vectors_are_reduced_and_positive(
    entries: list[tuple[str, dict[str, int]]],
) -> None:
    for group in pi_groups(dict(entries)):
        values = [value for value in group.values() if value != 0]
        assert values
        assert values[0] > 0
        assert reduce(gcd, (abs(value) for value in values)) == 1


def test_reordered_inputs_preserve_count_not_basis_identity() -> None:
    quantities = reynolds_quantities()

    assert count_pi_groups(quantities) == count_pi_groups(dict(reversed(quantities.items())))


def test_pi_groups_do_not_replace_kind_layer() -> None:
    registry = KindRegistry(
        kinds=[
            QuantityKind("Energy", ENERGY),
            QuantityKind("Torque", ENERGY),
            QuantityKind("Angle", {}),
        ],
        rules=[OperationRule("Energy", "div", "Torque", "Angle")],
    )
    energy_symbol, torque_symbol = sp.symbols("E tau")

    groups = pi_groups({"E": ENERGY, "tau": ENERGY, "theta": {}})

    assert len(groups) == 2
    assert all(
        is_dimensionless_product({"E": ENERGY, "tau": ENERGY, "theta": {}}, group)
        for group in groups
    )
    assert not verify_expr_kinds(
        sp.Eq(energy_symbol, torque_symbol),
        registry=registry,
        kind_map={"E": "Energy", "tau": "Torque"},
    )
